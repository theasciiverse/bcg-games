//! Golf: nine holes of our own, seen from above. Pick a club, aim, choose
//! how hard to swing, and mind the wind, the rough, the sand and the
//! water. Rules in asciiverse/specs/golf.md.

use crate::audio::Sound;
use crate::rng::Rng;
use crate::shell::{Ctx, Game, Key, Mouse, MouseKind};
use crate::store::Kv;
use crate::ui::*;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

/// Positions are in yards: the tee at (0, 0), the hole running toward +x,
/// +y to the left of the line of play (north on the map).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct P {
    pub x: f32,
    pub y: f32,
}

impl P {
    pub fn new(x: f32, y: f32) -> P {
        P { x, y }
    }
    pub fn dist(self, o: P) -> f32 {
        ((self.x - o.x).powi(2) + (self.y - o.y).powi(2)).sqrt()
    }
    pub fn angle_to(self, o: P) -> f32 {
        (o.y - self.y).atan2(o.x - self.x)
    }
    pub fn step(self, ang: f32, d: f32) -> P {
        P::new(self.x + ang.cos() * d, self.y + ang.sin() * d)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Lie {
    Tee,
    Fairway,
    Rough,
    Trees,
    Sand,
    Water,
    Green,
    Out,
}

impl Lie {
    pub fn name(self) -> &'static str {
        match self {
            Lie::Tee => "the tee",
            Lie::Fairway => "the fairway",
            Lie::Rough => "the rough",
            Lie::Trees => "the trees",
            Lie::Sand => "a bunker",
            Lie::Water => "the water",
            Lie::Green => "the green",
            Lie::Out => "out of bounds",
        }
    }
    /// How much of a club's distance this lie lets you have, and how much
    /// wider the shot spreads.
    fn penalty(self, club: usize) -> (f32, f32) {
        match self {
            Lie::Tee | Lie::Fairway | Lie::Green => (1.0, 1.0),
            Lie::Rough => (0.82, 1.5),
            Lie::Trees => (0.55, 2.4),
            Lie::Sand => {
                if club == SAND_WEDGE {
                    (0.9, 1.3)
                } else {
                    (0.6, 2.2)
                }
            }
            Lie::Water | Lie::Out => (1.0, 1.0),
        }
    }
}

pub struct Club {
    pub name: &'static str,
    pub short: &'static str,
    /// full-swing distance, carry and roll together, in yards
    pub yards: f32,
    /// how far a full swing strays, in degrees, before power and lie
    pub spread: f32,
}

pub const CLUBS: [Club; 10] = [
    Club {
        name: "Driver",
        short: "Dr",
        yards: 250.0,
        spread: 3.6,
    },
    Club {
        name: "3 wood",
        short: "3W",
        yards: 225.0,
        spread: 3.2,
    },
    Club {
        name: "5 wood",
        short: "5W",
        yards: 205.0,
        spread: 3.0,
    },
    Club {
        name: "3 iron",
        short: "3i",
        yards: 190.0,
        spread: 2.8,
    },
    Club {
        name: "5 iron",
        short: "5i",
        yards: 170.0,
        spread: 2.5,
    },
    Club {
        name: "7 iron",
        short: "7i",
        yards: 150.0,
        spread: 2.2,
    },
    Club {
        name: "9 iron",
        short: "9i",
        yards: 130.0,
        spread: 2.0,
    },
    Club {
        name: "Pitching wedge",
        short: "PW",
        yards: 110.0,
        spread: 1.8,
    },
    Club {
        name: "Sand wedge",
        short: "SW",
        yards: 80.0,
        spread: 1.8,
    },
    Club {
        name: "Putter",
        short: "Pt",
        yards: 30.0,
        spread: 1.2,
    },
];
pub const SAND_WEDGE: usize = 8;
pub const PUTTER: usize = 9;
pub const CUP: f32 = 0.3;
pub const GREEN_R: f32 = 16.0;
pub const MAX_STROKES: u32 = 10;

pub struct Hole {
    pub name: &'static str,
    pub par: u32,
    /// the line of play: the tee, any bend, and the pin last
    pub line: &'static [(f32, f32)],
    /// half the fairway's width
    pub fairway: f32,
    /// bunkers: centre and radius
    pub sand: &'static [(f32, f32, f32)],
    /// ponds and streams: centre and half-sizes of an ellipse
    pub water: &'static [(f32, f32, f32, f32)],
}

pub const COURSE: [Hole; 9] = [
    Hole {
        name: "Gentle Start",
        par: 4,
        line: &[(0.0, 0.0), (360.0, 0.0)],
        fairway: 24.0,
        sand: &[(250.0, -30.0, 12.0), (345.0, 22.0, 9.0)],
        water: &[],
    },
    Hole {
        name: "Mill Pond",
        par: 3,
        line: &[(0.0, 0.0), (165.0, 10.0)],
        fairway: 18.0,
        sand: &[(170.0, -22.0, 8.0)],
        water: &[(95.0, 2.0, 32.0, 26.0)],
    },
    Hole {
        name: "Long Meadow",
        par: 5,
        line: &[(0.0, 0.0), (270.0, 0.0), (500.0, 40.0)],
        fairway: 26.0,
        sand: &[
            (285.0, -32.0, 12.0),
            (420.0, 50.0, 10.0),
            (510.0, 18.0, 8.0),
        ],
        water: &[],
    },
    Hole {
        name: "Dogleg Left",
        par: 4,
        line: &[(0.0, 0.0), (240.0, 0.0), (390.0, 90.0)],
        fairway: 22.0,
        sand: &[(262.0, -28.0, 12.0), (380.0, 66.0, 8.0)],
        water: &[],
    },
    Hole {
        name: "Island Green",
        par: 3,
        line: &[(0.0, 0.0), (140.0, 0.0)],
        fairway: 12.0,
        sand: &[],
        water: &[(110.0, 0.0, 60.0, 40.0)],
    },
    Hole {
        name: "Creek Crossing",
        par: 4,
        line: &[(0.0, 0.0), (400.0, -20.0)],
        fairway: 24.0,
        sand: &[(395.0, 10.0, 9.0)],
        water: &[(285.0, -10.0, 10.0, 90.0)],
    },
    Hole {
        name: "Dogleg Right",
        par: 4,
        line: &[(0.0, 0.0), (230.0, 0.0), (370.0, -85.0)],
        fairway: 22.0,
        sand: &[(250.0, 30.0, 12.0), (350.0, -62.0, 8.0)],
        water: &[(300.0, -20.0, 18.0, 14.0)],
    },
    Hole {
        name: "The Long Way Home",
        par: 5,
        line: &[(0.0, 0.0), (250.0, 30.0), (520.0, -10.0)],
        fairway: 26.0,
        sand: &[
            (230.0, -6.0, 10.0),
            (500.0, -36.0, 10.0),
            (505.0, 20.0, 8.0),
        ],
        water: &[(410.0, 40.0, 30.0, 18.0)],
    },
    Hole {
        name: "Clubhouse",
        par: 4,
        line: &[(0.0, 0.0), (380.0, 0.0)],
        fairway: 22.0,
        sand: &[
            (245.0, 28.0, 11.0),
            (250.0, -28.0, 11.0),
            (370.0, -22.0, 8.0),
        ],
        water: &[],
    },
];

pub fn par_total() -> u32 {
    COURSE.iter().map(|h| h.par).sum()
}

fn noise(x: f32, y: f32) -> f32 {
    let xi = (x / 9.0).floor() as i64;
    let yi = (y / 9.0).floor() as i64;
    let mut h = (xi.wrapping_mul(73_856_093) ^ yi.wrapping_mul(19_349_663)) as u64;
    h ^= h >> 13;
    h = h.wrapping_mul(0x5bd1_e995);
    h ^= h >> 15;
    (h % 1000) as f32 / 1000.0
}

fn seg_dist(p: P, a: P, b: P) -> f32 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let l2 = dx * dx + dy * dy;
    let t = if l2 > 0.0 {
        (((p.x - a.x) * dx + (p.y - a.y) * dy) / l2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    p.dist(P::new(a.x + t * dx, a.y + t * dy))
}

impl Hole {
    pub fn pin(&self) -> P {
        let (x, y) = self.line[self.line.len() - 1];
        P::new(x, y)
    }

    pub fn yards(&self) -> f32 {
        self.line
            .windows(2)
            .map(|w| P::new(w[0].0, w[0].1).dist(P::new(w[1].0, w[1].1)))
            .sum()
    }

    pub fn off_line(&self, p: P) -> f32 {
        self.line
            .windows(2)
            .map(|w| seg_dist(p, P::new(w[0].0, w[0].1), P::new(w[1].0, w[1].1)))
            .fold(f32::MAX, f32::min)
    }

    /// The ground at a point.
    pub fn lie(&self, p: P) -> Lie {
        let pin = self.pin();
        let minx = self.line.iter().map(|l| l.0).fold(f32::MAX, f32::min);
        let maxx = self.line.iter().map(|l| l.0).fold(f32::MIN, f32::max);
        if p.x < minx - 30.0 || p.x > maxx + 45.0 {
            return Lie::Out;
        }
        if p.dist(pin) <= GREEN_R {
            return Lie::Green;
        }
        for &(cx, cy, rx, ry) in self.water {
            if ((p.x - cx) / rx).powi(2) + ((p.y - cy) / ry).powi(2) <= 1.0 {
                return Lie::Water;
            }
        }
        for &(cx, cy, r) in self.sand {
            if p.dist(P::new(cx, cy)) <= r {
                return Lie::Sand;
            }
        }
        if p.dist(P::new(0.0, 0.0)) <= 7.0 {
            return Lie::Tee;
        }
        let d = self.off_line(p);
        if d <= self.fairway {
            Lie::Fairway
        } else if d <= self.fairway + 22.0 {
            Lie::Rough
        } else if d <= self.fairway + 60.0 {
            if noise(p.x, p.y) < 0.55 {
                Lie::Trees
            } else {
                Lie::Rough
            }
        } else {
            Lie::Out
        }
    }

    /// A point on the line of play `ahead` yards on from the point of it
    /// nearest `p`, never past the pin.
    pub fn along(&self, p: P, ahead: f32) -> P {
        // how far along the line the nearest point is
        let mut best = (f32::MAX, 0.0f32);
        let mut run = 0.0;
        for w in self.line.windows(2) {
            let a = P::new(w[0].0, w[0].1);
            let b = P::new(w[1].0, w[1].1);
            let l = a.dist(b);
            let (dx, dy) = (b.x - a.x, b.y - a.y);
            let t = (((p.x - a.x) * dx + (p.y - a.y) * dy) / (l * l)).clamp(0.0, 1.0);
            let q = P::new(a.x + t * dx, a.y + t * dy);
            let d = p.dist(q);
            if d < best.0 {
                best = (d, run + t * l);
            }
            run += l;
        }
        let mut want = (best.1 + ahead).min(run);
        for w in self.line.windows(2) {
            let a = P::new(w[0].0, w[0].1);
            let b = P::new(w[1].0, w[1].1);
            let l = a.dist(b);
            if want <= l {
                return a.step(a.angle_to(b), want);
            }
            want -= l;
        }
        self.pin()
    }
}

/// What a swing does, before the ball meets the ground.
#[derive(Clone, Debug)]
pub struct Shot {
    pub path: Vec<P>,
    pub curve: &'static str,
}

/// The flight of a ball from `from`, aimed at `aim` radians, with a club
/// and a power (0.1 to 1.0), in a wind of (speed mph, toward radians).
pub fn strike(
    rng: &mut Rng,
    from: P,
    aim: f32,
    club: usize,
    power: f32,
    lie: Lie,
    wind: (f32, f32),
) -> Shot {
    let c = &CLUBS[club];
    let (dist_k, spread_k) = lie.penalty(club);
    let mut dist = c.yards * power * dist_k * (0.99 + rng.normal() * 0.045).clamp(0.8, 1.1);
    let mut dev = rng.normal() * c.spread * 1.5 * spread_k * (0.5 + 0.5 * power);
    let mut curve = "";
    if club != PUTTER {
        // the harder the swing, the likelier a hook or a slice
        let p = 0.05 + 0.4 * (power - 0.5).max(0.0) * spread_k.min(2.0);
        if rng.chance(p) {
            let amount = rng.between(5.0, 13.0);
            if rng.chance(0.5) {
                dev += amount;
                curve = "hooks";
            } else {
                dev -= amount;
                curve = "slices";
            }
            dist *= 0.92;
        }
    } else {
        dev = rng.normal() * (3.5 + c.spread * power);
        dist = c.yards * power * (1.0 + rng.normal() * 0.07);
    }
    let heading = aim + dev.to_radians();
    let steps = 24;
    let mut path = Vec::with_capacity(steps + 1);
    let (ws, wa) = wind;
    // for a putt, `wind` is the green's slope: yards of break per yard rolled
    let drift = if club == PUTTER {
        ws * dist
    } else {
        ws * 0.55 * dist / 100.0
    };
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        // the bend of a hook or slice grows through the flight
        let p = from.step(heading, dist * t);
        let w = drift * t * t;
        path.push(P::new(p.x + wa.cos() * w, p.y + wa.sin() * w));
    }
    Shot { path, curve }
}

/// The club whose full swing first covers `d`, or the driver.
pub fn club_for(d: f32, lie: Lie) -> usize {
    if lie == Lie::Green {
        return PUTTER;
    }
    if lie == Lie::Sand && d < 95.0 {
        return SAND_WEDGE;
    }
    for i in (0..PUTTER).rev() {
        if CLUBS[i].yards * lie.penalty(i).0 >= d {
            return i;
        }
    }
    0
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Phase {
    Aiming,
    Flying,
    Holed,
    RoundOver,
}

pub struct Golf {
    ctx: Ctx,
    hole: usize,
    ball: P,
    lie: Lie,
    club: usize,
    power: f32,
    aim: f32,
    wind: (f32, f32),
    /// the green's slope: yards of break per yard rolled, and its direction
    slope: (f32, f32),
    strokes: u32,
    card: Vec<u32>,
    phase: Phase,
    flight: Option<(Shot, P)>,
    fly_t: f32,
    trail: Vec<P>,
    note: String,
    stats: Kv,
    show_help: bool,
    quit: bool,
    map: Rect,
    view: (P, f32, f32),
    buttons: Vec<(Rect, Key)>,
    demo_clock: f32,
}

impl Golf {
    fn start_round(&mut self) {
        self.card.clear();
        self.hole = 0;
        log::info!("new round");
        self.start_hole();
    }

    fn start_hole(&mut self) {
        let h = &COURSE[self.hole];
        self.ball = P::new(0.0, 0.0);
        self.lie = Lie::Tee;
        self.strokes = 0;
        self.trail.clear();
        self.slope = (
            self.ctx.rng.between(0.015, 0.07),
            self.ctx.rng.between(0.0, std::f32::consts::TAU),
        );
        self.wind = (
            self.ctx.rng.range(0, 14) as f32,
            self.ctx.rng.between(0.0, std::f32::consts::TAU),
        );
        self.phase = Phase::Aiming;
        self.note = format!(
            "Hole {}, {}: par {}, {} yards.",
            self.hole + 1,
            h.name,
            h.par,
            h.yards().round()
        );
        log::info!(
            "hole {} wind {:.0} mph toward {:.2}",
            self.hole + 1,
            self.wind.0,
            self.wind.1
        );
        self.caddie();
    }

    /// The caddie's suggestion: a club, a target on the line of play, and
    /// the power to reach it.
    fn caddie(&mut self) {
        let h = &COURSE[self.hole];
        let pin = h.pin();
        let to_pin = self.ball.dist(pin);
        let reach = CLUBS[0].yards * self.lie.penalty(0).0;
        let target = if to_pin <= reach * 1.02 || self.lie == Lie::Green {
            pin
        } else {
            h.along(self.ball, reach * 0.95)
        };
        let d = self.ball.dist(target);
        let club = club_for(d, self.lie);
        self.club = club;
        self.aim = self.ball.angle_to(target);
        let full = CLUBS[club].yards * self.lie.penalty(club).0;
        if club == PUTTER {
            // the caddie reads about half the break
            let (k, a) = self.slope;
            let t = P::new(pin.x - a.cos() * k * d * 0.5, pin.y - a.sin() * k * d * 0.5);
            self.aim = self.ball.angle_to(t);
            self.power = ((d + 0.4) / full).clamp(0.02, 1.0);
            self.power = (self.power * 100.0).round() / 100.0;
        } else {
            self.power = (d / full).clamp(0.05, 1.0);
            self.power = ((self.power * 20.0).round() / 20.0).max(0.05);
        }
    }

    fn swing(&mut self) {
        if self.phase != Phase::Aiming {
            return;
        }
        let shot = strike(
            &mut self.ctx.rng,
            self.ball,
            self.aim,
            self.club,
            self.power,
            self.lie,
            if self.club == PUTTER {
                self.slope
            } else {
                self.wind
            },
        );
        self.strokes += 1;
        log::info!(
            "swing {} power {:.2} aim {:.3} from {:?} {:?}",
            CLUBS[self.club].name,
            self.power,
            self.aim,
            self.ball,
            self.lie
        );
        self.ctx.audio.play(if self.club == PUTTER {
            Sound::Tick
        } else {
            Sound::Hiss(self.power)
        });
        let start = self.ball;
        self.flight = Some((shot, start));
        self.fly_t = 0.0;
        self.trail.clear();
        self.phase = Phase::Flying;
    }

    fn land(&mut self) {
        let Some((shot, start)) = self.flight.take() else {
            return;
        };
        let h = &COURSE[self.hole];
        let pin = h.pin();
        let end = *shot.path.last().unwrap();
        // a putt or a chip that runs over the cup at a gentle pace drops
        let mut holed = end.dist(pin) <= CUP;
        if !holed && (self.club == PUTTER || start.dist(pin) < 40.0) {
            for w in shot.path.windows(2) {
                let catch = if self.club == PUTTER {
                    CUP * 0.8
                } else {
                    CUP * 0.4
                };
                if seg_dist(pin, w[0], w[1]) <= catch {
                    let past = end.dist(pin);
                    if past < 1.5 {
                        holed = true;
                    }
                    break;
                }
            }
        }
        let lie = h.lie(end);
        let mut note;
        if holed {
            self.ball = pin;
            self.phase = Phase::Holed;
            let diff = self.strokes as i32 - h.par as i32;
            let word = match diff {
                i32::MIN..=-3 => "An albatross!",
                -2 => "An eagle!",
                -1 => "A birdie!",
                0 => "Par.",
                1 => "A bogey.",
                2 => "A double bogey.",
                _ => "Holed at last.",
            };
            note = if self.strokes == 1 {
                "A hole in one!".to_string()
            } else {
                format!("In the hole. {word}")
            };
            self.ctx
                .audio
                .play(if diff <= 0 { Sound::Win } else { Sound::Good });
            self.card.push(self.strokes);
            note += "  Enter for the next hole.";
        } else if lie == Lie::Water || lie == Lie::Out {
            // a stroke added; water drops the ball back where it went in,
            // out of bounds plays again from the same spot
            self.strokes += 1;
            if lie == Lie::Water {
                let mut drop = start;
                for p in &shot.path {
                    if h.lie(*p) == Lie::Water {
                        break;
                    }
                    drop = *p;
                }
                self.ball = drop;
                self.lie = h.lie(drop);
                note = format!("Splash. One stroke added; you drop on {}.", self.lie.name());
            } else {
                self.ball = start;
                note = "Out of bounds. One stroke added; play again from there.".to_string();
            }
            self.ctx.audio.play(Sound::Bad);
            self.phase = Phase::Aiming;
        } else {
            self.ball = end;
            self.lie = lie;
            let left = end.dist(pin);
            note = if lie == Lie::Green {
                if left < 2.0 {
                    format!("{:.0} feet from the cup.", (left * 3.0).max(1.0))
                } else {
                    format!("On the green, {:.0} feet away.", left * 3.0)
                }
            } else {
                format!("In {}, {:.0} yards to go.", lie.name(), left)
            };
            if !shot.curve.is_empty() {
                note = format!("It {} away. {note}", shot.curve);
            }
            self.ctx.audio.play(match lie {
                Lie::Fairway | Lie::Green => Sound::Click,
                _ => Sound::Nope,
            });
            self.phase = Phase::Aiming;
        }
        if self.phase == Phase::Aiming && self.strokes >= MAX_STROKES {
            note = format!("That's {MAX_STROKES}; pick up and move on. Enter for the next hole.");
            self.card.push(MAX_STROKES);
            self.phase = Phase::Holed;
        }
        self.note = note;
        log::info!(
            "landed {:?} {:?} strokes {} phase {:?}",
            self.ball,
            self.lie,
            self.strokes,
            self.phase
        );
        if self.phase == Phase::Aiming {
            self.caddie();
        }
    }

    fn next(&mut self) {
        match self.phase {
            Phase::Aiming => self.swing(),
            Phase::Holed => {
                if self.hole + 1 < COURSE.len() {
                    self.hole += 1;
                    self.start_hole();
                } else {
                    self.finish();
                }
            }
            Phase::RoundOver => self.start_round(),
            Phase::Flying => {}
        }
    }

    fn finish(&mut self) {
        self.phase = Phase::RoundOver;
        let total: u32 = self.card.iter().sum();
        let diff = total as i32 - par_total() as i32;
        let best = self.stats.get::<u32>("best");
        let new_best = best.is_none_or(|b| total < b);
        self.note = format!(
            "Round over: {total}, {}.{}",
            over_under(diff),
            if new_best && !self.ctx.demo {
                " Your best round yet!"
            } else {
                ""
            }
        );
        self.ctx.audio.play(Sound::Win);
        log::info!("round over {total}");
        if !self.ctx.demo {
            let n = self.stats.get::<u32>("rounds").unwrap_or(0) + 1;
            self.stats.set("rounds", n);
            if new_best {
                self.stats.set("best", total);
            }
            let holes_in_one = self.card.iter().filter(|s| **s == 1).count() as u32;
            if holes_in_one > 0 {
                let a = self.stats.get::<u32>("aces").unwrap_or(0) + holes_in_one;
                self.stats.set("aces", a);
            }
            self.stats.save(Self::ID, "scores");
        }
    }

    fn step(&self) -> f32 {
        if self.club == PUTTER {
            0.02
        } else {
            0.05
        }
    }

    fn turn_aim(&mut self, deg: f32) {
        self.aim += deg.to_radians();
        self.ctx.audio.play(Sound::Tick);
    }

    fn set_club(&mut self, c: usize) {
        if self.lie == Lie::Green && c != PUTTER {
            self.note = "On the green, it's the putter.".into();
            self.ctx.audio.play(Sound::Nope);
            return;
        }
        self.club = c;
        self.ctx.audio.play(Sound::Tick);
    }

    fn map_to_screen(&self, p: P) -> Option<(u16, u16)> {
        let (c, xs, ys) = self.view;
        let m = self.map;
        let fx = (p.x - c.x) / xs + m.width as f32 / 2.0;
        let fy = -(p.y - c.y) / ys + m.height as f32 / 2.0;
        if fx < 0.0 || fy < 0.0 || fx >= m.width as f32 || fy >= m.height as f32 {
            return None;
        }
        Some((m.x + fx as u16, m.y + fy as u16))
    }

    fn screen_to_map(&self, x: u16, y: u16) -> P {
        let (c, xs, ys) = self.view;
        let m = self.map;
        let fx = x as f32 - m.x as f32 + 0.5 - m.width as f32 / 2.0;
        let fy = y as f32 - m.y as f32 + 0.5 - m.height as f32 / 2.0;
        P::new(c.x + fx * xs, c.y - fy * ys)
    }

    /// Choose the view: the whole hole, or the green up close.
    fn set_view(&mut self) {
        let h = &COURSE[self.hole];
        let m = self.map;
        let (w, hgt) = (m.width.max(1) as f32, m.height.max(1) as f32);
        let near_green = self.ball.dist(h.pin()) < 30.0 && self.phase != Phase::RoundOver;
        if near_green {
            let mid = P::new(
                (self.ball.x + h.pin().x) / 2.0,
                (self.ball.y + h.pin().y) / 2.0,
            );
            let span = (self.ball.dist(h.pin()) + 14.0).max(36.0);
            let xs = (span / w).max(0.5);
            self.view = (mid, xs, xs * 2.0);
            return;
        }
        let minx = h.line.iter().map(|l| l.0).fold(f32::MAX, f32::min) - 25.0;
        let maxx = h.line.iter().map(|l| l.0).fold(f32::MIN, f32::max) + 35.0;
        let miny = h.line.iter().map(|l| l.1).fold(f32::MAX, f32::min) - 60.0;
        let maxy = h.line.iter().map(|l| l.1).fold(f32::MIN, f32::max) + 60.0;
        let xs = ((maxx - minx) / w).max((maxy - miny) / hgt / 2.0);
        let c = P::new((minx + maxx) / 2.0, (miny + maxy) / 2.0);
        self.view = (c, xs, xs * 2.0);
    }

    fn demo_tick(&mut self, dt: f32) {
        self.demo_clock += dt;
        match self.phase {
            Phase::Aiming if self.demo_clock > 1.3 => {
                self.demo_clock = 0.0;
                self.caddie();
                self.swing();
            }
            Phase::Holed if self.demo_clock > 2.2 => {
                self.demo_clock = 0.0;
                self.next();
            }
            Phase::RoundOver if self.demo_clock > 4.0 => {
                self.demo_clock = 0.0;
                self.start_round();
            }
            _ => {}
        }
    }
}

pub fn over_under(diff: i32) -> String {
    match diff {
        0 => "level par".into(),
        d if d > 0 => format!("{d} over par"),
        d => format!("{} under par", -d),
    }
}

fn ground_style(l: Lie, x: f32, y: f32) -> (&'static str, Color, Color) {
    let n = noise(x * 3.1, y * 2.3);
    match l {
        Lie::Fairway => ("░", Color::Rgb(70, 150, 60), Color::Rgb(44, 110, 40)),
        Lie::Tee => ("▒", Color::Rgb(110, 190, 90), Color::Rgb(50, 120, 45)),
        Lie::Rough => (
            if n < 0.3 { "\"" } else { " " },
            Color::Rgb(70, 120, 50),
            Color::Rgb(30, 74, 30),
        ),
        Lie::Trees => (
            if n < 0.5 { "♣" } else { "♠" },
            Color::Rgb(40, 140, 60),
            Color::Rgb(16, 46, 20),
        ),
        Lie::Sand => ("░", Color::Rgb(230, 210, 150), Color::Rgb(200, 180, 120)),
        Lie::Water => (
            if n < 0.25 { "~" } else { " " },
            Color::Rgb(150, 200, 240),
            Color::Rgb(40, 90, 170),
        ),
        Lie::Green => (" ", Color::Rgb(120, 220, 110), Color::Rgb(70, 170, 70)),
        Lie::Out => ("·", Color::Rgb(50, 60, 50), Color::Rgb(14, 22, 14)),
    }
}

impl Game for Golf {
    const ID: &'static str = "golf";
    const TITLE: &'static str = "Golf";
    const HELP: &'static str =
        "Nine holes, par 36, seen from above. The caddie picks a club, an aim and a
swing for every shot; change any of them and swing. Wind carries the ball,
rough and sand and trees take distance away, hard swings hook or slice now
and then, and water or out of bounds costs a stroke.
Up and Down change the club, Left and Right aim (comma and full stop fine
tune), + and - or 1 to 0 set the swing (10 to 100 percent), a click on the
map aims there. Enter or Space swings. C asks the caddie again, N starts a
new round, Tab shows the rules, M sound, Esc quits.";

    fn new(ctx: Ctx) -> Self {
        let mut g = Golf {
            ctx,
            hole: 0,
            ball: P::new(0.0, 0.0),
            lie: Lie::Tee,
            club: 0,
            power: 1.0,
            aim: 0.0,
            wind: (0.0, 0.0),
            slope: (0.0, 0.0),
            strokes: 0,
            card: Vec::new(),
            phase: Phase::Aiming,
            flight: None,
            fly_t: 0.0,
            trail: Vec::new(),
            note: String::new(),
            stats: Kv::load("golf", "scores"),
            show_help: false,
            quit: false,
            map: Rect::new(0, 0, 60, 20),
            view: (P::new(0.0, 0.0), 6.0, 12.0),
            buttons: Vec::new(),
            demo_clock: 0.0,
        };
        g.start_round();
        g
    }

    fn key(&mut self, k: Key) {
        if self.ctx.demo {
            if matches!(k, Key::Esc | Key::Char('q')) {
                self.quit = true;
            }
            return;
        }
        let aiming = self.phase == Phase::Aiming;
        match k {
            Key::Esc | Key::Char('q') => self.quit = true,
            Key::Tab | Key::Char('?') => self.show_help = !self.show_help,
            Key::Char('m') => {
                self.ctx.audio.toggle();
                crate::shell::remember_mute(Self::ID, self.ctx.audio.muted);
            }
            Key::Char('n') => {
                self.start_round();
                self.ctx.audio.play(Sound::Click);
            }
            Key::Enter | Key::Char(' ') => self.next(),
            Key::Up if aiming => {
                if self.club > 0 {
                    self.set_club(self.club - 1);
                }
            }
            Key::Down if aiming => {
                if self.club < PUTTER {
                    self.set_club(self.club + 1);
                }
            }
            Key::Left if aiming => self.turn_aim(2.0),
            Key::Right if aiming => self.turn_aim(-2.0),
            Key::Char(',') if aiming => self.turn_aim(0.5),
            Key::Char('.') if aiming => self.turn_aim(-0.5),
            Key::Char('+') | Key::Char('=') | Key::PageUp if aiming => {
                self.power = (self.power + self.step()).min(1.0);
                self.ctx.audio.play(Sound::Tick);
            }
            Key::Char('-') | Key::PageDown if aiming => {
                self.power = (self.power - self.step()).max(0.02);
                self.ctx.audio.play(Sound::Tick);
            }
            Key::Char(c) if aiming && c.is_ascii_digit() => {
                let d = c as u8 - b'0';
                self.power = if d == 0 { 1.0 } else { d as f32 / 10.0 };
                self.ctx.audio.play(Sound::Tick);
            }
            Key::Char('c') if aiming => {
                self.caddie();
                self.ctx.audio.play(Sound::Click);
            }
            _ => {}
        }
    }

    fn mouse(&mut self, m: Mouse) {
        if self.ctx.demo {
            return;
        }
        match m.kind {
            MouseKind::Click => {
                if let Some((_, k)) = self.buttons.iter().find(|(r, _)| hit(r, m.x, m.y)).cloned() {
                    self.key(k);
                    return;
                }
                if self.phase == Phase::Aiming && hit(&self.map, m.x, m.y) {
                    // aim at the spot, and swing just hard enough to get there
                    let target = self.screen_to_map(m.x, m.y);
                    let d = self.ball.dist(target);
                    self.aim = self.ball.angle_to(target);
                    let full = CLUBS[self.club].yards * self.lie.penalty(self.club).0;
                    if d > full && self.lie != Lie::Green {
                        self.club = club_for(d, self.lie);
                    }
                    let full = CLUBS[self.club].yards * self.lie.penalty(self.club).0;
                    self.power = if self.club == PUTTER {
                        (((d + 0.4) / full) * 100.0).round().clamp(2.0, 100.0) / 100.0
                    } else {
                        ((d / full) * 20.0).round().clamp(1.0, 20.0) / 20.0
                    };
                    self.ctx.audio.play(Sound::Tick);
                }
            }
            MouseKind::ScrollUp if self.phase == Phase::Aiming => {
                self.power = (self.power + self.step()).min(1.0)
            }
            MouseKind::ScrollDown if self.phase == Phase::Aiming => {
                self.power = (self.power - self.step()).max(0.02)
            }
            _ => {}
        }
    }

    fn tick(&mut self, dt: f32) {
        if self.phase == Phase::Flying {
            let dur = if self.club == PUTTER { 1.0 } else { 1.4 };
            self.fly_t += dt / dur;
            if let Some((shot, _)) = &self.flight {
                let n = shot.path.len() - 1;
                let i = ((self.fly_t.min(1.0)) * n as f32) as usize;
                self.trail = shot.path[..=i.min(n)].to_vec();
            }
            if self.fly_t >= 1.0 {
                self.land();
                self.demo_clock = 0.0;
            }
        }
        if self.ctx.demo {
            self.demo_tick(dt);
        }
    }

    fn quit(&self) -> bool {
        self.quit
    }

    fn snapshot(&self) -> String {
        format!(
            "phase {:?} hole {} strokes {} ball {:.0},{:.0} lie {:?} club {} power {:.2} card {:?}",
            self.phase,
            self.hole + 1,
            self.strokes,
            self.ball.x,
            self.ball.y,
            self.lie,
            CLUBS[self.club].short,
            self.power,
            self.card
        )
    }

    fn draw(&mut self, area: Rect, buf: &mut Buffer) {
        self.buttons.clear();
        let total: u32 = self.card.iter().sum();
        let played_par: u32 = COURSE.iter().take(self.card.len()).map(|h| h.par).sum();
        let status = format!(
            "hole {} of 9   {}",
            (self.hole + 1).min(9),
            if self.card.is_empty() {
                "level".to_string()
            } else {
                over_under(total as i32 - played_par as i32)
            }
        );
        let inner = frame(
            buf,
            area,
            Self::TITLE,
            &status,
            &[
                ("↑↓", "club"),
                ("←→", "aim"),
                ("+-", "swing"),
                ("Enter", "swing!"),
                ("C", "caddie"),
                ("N", "new"),
                ("Tab", "rules"),
                ("M", "sound"),
                ("Esc", "quit"),
            ],
        );
        if too_small(buf, inner, 76, 22) {
            return;
        }
        let side_w = 28u16;
        let card_h = 5u16;
        let left = Rect {
            width: inner.width - side_w,
            height: inner.height - card_h,
            ..inner
        };
        let side = Rect {
            x: inner.x + left.width,
            width: side_w,
            height: inner.height - card_h,
            ..inner
        };
        let card_r = Rect {
            y: inner.bottom() - card_h,
            height: card_h,
            ..inner
        };

        // the hole, from above
        let h = &COURSE[self.hole];
        let mp = panel(
            buf,
            left,
            &format!("Hole {}: {}", self.hole + 1, h.name),
            AMBER,
        );
        let map = Rect {
            height: mp.height.saturating_sub(1),
            ..mp
        };
        self.map = map;
        if self.phase != Phase::Flying {
            self.set_view();
        }
        for y in map.top()..map.bottom() {
            for x in map.left()..map.right() {
                let p = self.screen_to_map(x, y);
                let l = h.lie(p);
                let (sym, fg, bg) = ground_style(l, p.x, p.y);
                buf[(x, y)].set_symbol(sym).set_style(on(fg, bg));
            }
        }
        // the aim line, while aiming
        if self.phase == Phase::Aiming {
            let full = CLUBS[self.club].yards * self.lie.penalty(self.club).0 * self.power;
            let n = 40;
            for i in 1..=n {
                let p = self.ball.step(self.aim, full * i as f32 / n as f32);
                if let Some((x, y)) = self.map_to_screen(p) {
                    let cell = &mut buf[(x, y)];
                    if i == n {
                        cell.set_symbol("✕").set_fg(AMBER);
                    } else if i % 3 == 0 {
                        cell.set_symbol("·").set_fg(Color::Rgb(255, 230, 150));
                    }
                }
            }
        }
        // the pin and the ball, and its flight
        if let Some((x, y)) = self.map_to_screen(h.pin()) {
            buf[(x, y)].set_symbol("⚑").set_fg(RED);
        }
        for p in &self.trail {
            if let Some((x, y)) = self.map_to_screen(*p) {
                buf[(x, y)]
                    .set_symbol("∙")
                    .set_fg(Color::Rgb(250, 250, 250));
            }
        }
        let ball = if self.phase == Phase::Flying {
            self.trail.last().copied().unwrap_or(self.ball)
        } else {
            self.ball
        };
        if self.phase != Phase::Holed && self.phase != Phase::RoundOver {
            if let Some((x, y)) = self.map_to_screen(ball) {
                buf[(x, y)]
                    .set_symbol("●")
                    .set_fg(Color::Rgb(255, 255, 255));
            }
        }
        let note_y = mp.bottom() - 1;
        fill(buf, Rect::new(mp.x, note_y, mp.width, 1), st(INK));
        put_max(
            buf,
            mp.x,
            note_y,
            &self.note,
            bold(st(match self.phase {
                Phase::Holed | Phase::RoundOver => GREEN,
                _ => CYAN,
            })),
            mp.width,
        );

        // the shot
        if self.show_help {
            let p = panel(buf, side, "How it works", CYAN);
            para(
                buf,
                p,
                "The caddie sets up every shot; change the club, aim or swing and press Enter. A full swing goes the club's distance; the wind carries the ball, rough, sand and trees cost distance, and a hard swing can hook or slice. Water or out of bounds: one stroke added. Near the green the map comes in close.",
                st(INK),
            );
        } else {
            let p = panel(buf, side, "Your shot", CYAN);
            let to_pin = self.ball.dist(h.pin());
            let mut y = p.y;
            line(
                buf,
                p,
                &mut y,
                format!("par {}  {} yards", h.par, h.yards().round()),
                INK,
            );
            line(buf, p, &mut y, format!("stroke {}", self.strokes + 1), INK);
            line(buf, p, &mut y, format!("lie   {}", self.lie.name()), DIM);
            line(
                buf,
                p,
                &mut y,
                if self.lie == Lie::Green {
                    format!("pin   {:.0} feet", to_pin * 3.0)
                } else {
                    format!("pin   {:.0} yards", to_pin)
                },
                DIM,
            );
            let compass = wind_arrow(self.wind.1);
            if self.lie == Lie::Green {
                line(
                    buf,
                    p,
                    &mut y,
                    format!(
                        "green slopes {} {}",
                        wind_arrow(self.slope.1),
                        if self.slope.0 > 0.045 {
                            "firmly"
                        } else {
                            "gently"
                        }
                    ),
                    DIM,
                );
            }
            line(
                buf,
                p,
                &mut y,
                if self.wind.0 < 1.0 {
                    "wind  still".to_string()
                } else {
                    format!("wind  {} {:.0} mph", compass, self.wind.0)
                },
                DIM,
            );
            y += 1;
            let c = &CLUBS[self.club];
            if y < p.bottom() {
                put(buf, p.x, y, "club  ", st(DIM));
                put(buf, p.x + 6, y, c.name, bold(st(AMBER)));
            }
            y += 1;
            let reach = c.yards * self.lie.penalty(self.club).0;
            line(
                buf,
                p,
                &mut y,
                format!("      full {:.0} yards", reach),
                DIM,
            );
            if y < p.bottom() {
                put(buf, p.x, y, "swing ", st(DIM));
                let w = p.width.saturating_sub(12);
                bar(buf, p.x + 6, y, w, self.power, AMBER);
                put(
                    buf,
                    p.x + 7 + w,
                    y,
                    &format!("{:>3}%", (self.power * 100.0).round()),
                    st(INK),
                );
            }
            y += 1;
            let carry = reach * self.power;
            line(
                buf,
                p,
                &mut y,
                if self.club == PUTTER {
                    format!("      about {:.0} feet", carry * 3.0)
                } else {
                    format!("      about {:.0} yards", carry)
                },
                DIM,
            );
            y += 1;
            if y < p.bottom() {
                let label = match self.phase {
                    Phase::Aiming => " Swing ",
                    Phase::Flying => " ...   ",
                    Phase::Holed => " Next hole ",
                    Phase::RoundOver => " New round ",
                };
                let w = label.chars().count() as u16;
                let r = Rect::new(p.x, y, w, 1);
                fill(buf, r, on(BG, AMBER));
                put(buf, p.x, y, label, bold(on(BG, AMBER)));
                self.buttons.push((r, Key::Enter));
                if self.phase == Phase::Aiming {
                    let r2 = Rect::new(p.x + w + 2, y, 8, 1);
                    fill(buf, r2, on(INK, PAPER));
                    put(buf, r2.x, y, " Caddie ", bold(on(INK, PAPER)));
                    self.buttons.push((r2, Key::Char('c')));
                }
            }
            y += 1;
            let best = self
                .stats
                .get::<u32>("best")
                .map_or("-".into(), |b| b.to_string());
            line(
                buf,
                p,
                &mut y,
                format!(
                    "best round {best}  rounds {}",
                    self.stats.get::<u32>("rounds").unwrap_or(0)
                ),
                DIM,
            );
        }

        // the scorecard
        let sc = panel(buf, card_r, "Scorecard", GREEN);
        let col = 5u16;
        put(buf, sc.x, sc.y, "hole", st(DIM));
        put(buf, sc.x, sc.y + 1, "par", st(DIM));
        put(buf, sc.x, sc.y + 2, "you", st(DIM));
        for (i, hl) in COURSE.iter().enumerate() {
            let x = sc.x + 6 + i as u16 * col;
            let cur = i == self.hole && self.phase != Phase::RoundOver;
            put(
                buf,
                x,
                sc.y,
                &format!("{:>3}", i + 1),
                bold(st(if cur { AMBER } else { INK })),
            );
            put(buf, x, sc.y + 1, &format!("{:>3}", hl.par), st(DIM));
            let (s, c) = match self.card.get(i) {
                Some(&n) => (
                    format!("{n:>3}"),
                    if n < hl.par {
                        GREEN
                    } else if n == hl.par {
                        INK
                    } else {
                        RED
                    },
                ),
                None if cur && self.strokes > 0 => (format!("{:>3}", self.strokes), AMBER),
                None => ("  -".into(), LINE),
            };
            put(buf, x, sc.y + 2, &s, bold(st(c)));
        }
        let x = sc.x + 6 + 9 * col + 2;
        put(buf, x, sc.y, "total", st(DIM));
        put(buf, x, sc.y + 1, &format!("{:>5}", par_total()), st(DIM));
        put(buf, x, sc.y + 2, &format!("{total:>5}"), bold(st(AMBER)));
    }
}

fn line(buf: &mut Buffer, p: Rect, y: &mut u16, s: String, c: Color) {
    if *y < p.bottom() {
        put_max(buf, p.x, *y, &s, st(c), p.width);
    }
    *y += 1;
}

fn wind_arrow(a: f32) -> &'static str {
    // the map's +x is east (toward the green), +y north
    let i = ((a.to_degrees().rem_euclid(360.0) + 22.5) / 45.0) as usize % 8;
    ["→", "↗", "↑", "↖", "←", "↙", "↓", "↘"][i]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_course_is_par_36_and_every_pin_is_on_a_green() {
        assert_eq!(par_total(), 36);
        for h in &COURSE {
            assert_eq!(h.lie(h.pin()), Lie::Green);
            assert_eq!(h.lie(P::new(0.0, 0.0)), Lie::Tee);
        }
    }

    #[test]
    fn island_green_has_water_short_of_it() {
        let h = &COURSE[4];
        assert_eq!(h.lie(P::new(90.0, 0.0)), Lie::Water);
    }

    #[test]
    fn caddie_clubs_get_shorter_closer_in() {
        assert_eq!(club_for(240.0, Lie::Fairway), 0);
        assert_eq!(club_for(220.0, Lie::Fairway), 1);
        assert_eq!(club_for(140.0, Lie::Fairway), 5);
        assert_eq!(club_for(70.0, Lie::Sand), SAND_WEDGE);
        assert_eq!(club_for(5.0, Lie::Green), PUTTER);
    }

    #[test]
    fn a_soft_putt_goes_about_where_it_was_aimed() {
        let mut rng = Rng::new(9);
        let mut near = 0;
        for _ in 0..100 {
            let s = strike(
                &mut rng,
                P::new(0.0, 0.0),
                0.0,
                PUTTER,
                0.2,
                Lie::Green,
                (0.0, 0.0),
            );
            let end = *s.path.last().unwrap();
            if end.dist(P::new(6.0, 0.0)) < 1.5 {
                near += 1;
            }
        }
        assert!(near > 70, "{near}");
    }

    #[test]
    fn along_follows_the_dogleg() {
        let h = &COURSE[3];
        let p = h.along(P::new(0.0, 0.0), 300.0);
        assert!(p.y > 10.0 && p.x > 240.0);
    }
}
