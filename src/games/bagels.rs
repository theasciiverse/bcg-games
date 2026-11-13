//! Bagels: find three different digits from clues. Rules in
//! asciiverse/specs/bagels.md.

use crate::audio::Sound;
use crate::shell::{Ctx, Game, Key, Mouse, MouseKind};
use crate::store::Kv;
use crate::ui::*;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

pub const DIGITS: usize = 3;
pub const TRIES: usize = 20;

/// How a guess scores against the secret.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Clue {
    /// right digit, right place
    pub fermi: u8,
    /// right digit, wrong place
    pub pico: u8,
}

impl Clue {
    pub fn words(&self) -> Vec<&'static str> {
        let mut w = vec!["Fermi"; self.fermi as usize];
        w.extend(std::iter::repeat_n("Pico", self.pico as usize));
        if w.is_empty() {
            w.push("Bagels");
        }
        w
    }
}

pub fn score(secret: &[u8; DIGITS], guess: &[u8; DIGITS]) -> Clue {
    let mut c = Clue::default();
    for i in 0..DIGITS {
        if guess[i] == secret[i] {
            c.fermi += 1;
        } else if secret.contains(&guess[i]) {
            c.pico += 1;
        }
    }
    c
}

pub fn distinct(d: &[u8; DIGITS]) -> bool {
    d[0] != d[1] && d[0] != d[2] && d[1] != d[2]
}

/// Every possible secret.
pub fn all() -> Vec<[u8; DIGITS]> {
    let mut v = Vec::new();
    for a in 0..10 {
        for b in 0..10 {
            for c in 0..10 {
                let d = [a, b, c];
                if distinct(&d) {
                    v.push(d);
                }
            }
        }
    }
    v
}

/// The secrets still possible after these guesses.
pub fn candidates(history: &[([u8; DIGITS], Clue)]) -> Vec<[u8; DIGITS]> {
    all()
        .into_iter()
        .filter(|s| history.iter().all(|(g, c)| score(s, g) == *c))
        .collect()
}

#[derive(PartialEq, Debug)]
enum Phase {
    Guessing,
    Won,
    Lost,
}

pub struct Bagels {
    ctx: Ctx,
    secret: [u8; DIGITS],
    history: Vec<([u8; DIGITS], Clue)>,
    typed: Vec<u8>,
    phase: Phase,
    note: String,
    stats: Kv,
    quit: bool,
    show_help: bool,
    demo_clock: f32,
    demo_target: Option<[u8; DIGITS]>,
    buttons: Vec<(Rect, Key)>,
}

impl Bagels {
    fn deal(&mut self) {
        let pool = all();
        self.secret = pool[self.ctx.rng.range(0, pool.len() as i64 - 1) as usize];
        self.history.clear();
        self.typed.clear();
        self.phase = Phase::Guessing;
        self.note = "Three different digits. What's your first guess?".into();
        log::info!("new secret dealt");
    }

    fn submit(&mut self) {
        if self.typed.len() != DIGITS {
            self.note = format!("Type {DIGITS} digits first.");
            self.ctx.audio.play(Sound::Nope);
            return;
        }
        let g = [self.typed[0], self.typed[1], self.typed[2]];
        if !distinct(&g) {
            self.note = "No digit twice: the secret never repeats one.".into();
            self.ctx.audio.play(Sound::Nope);
            return;
        }
        let c = score(&self.secret, &g);
        self.history.push((g, c));
        self.typed.clear();
        log::info!("guess {:?} -> fermi {} pico {}", g, c.fermi, c.pico);
        if c.fermi as usize == DIGITS {
            self.phase = Phase::Won;
            self.note = format!("Got it in {}! Enter for another.", self.history.len());
            self.ctx.audio.play(Sound::Win);
            self.record(true);
        } else if self.history.len() >= TRIES {
            self.phase = Phase::Lost;
            let s: String = self.secret.iter().map(|d| char::from(b'0' + d)).collect();
            self.note = format!("Out of guesses. It was {s}. Enter for another.");
            self.ctx.audio.play(Sound::Lose);
            self.record(false);
        } else {
            self.note = String::new();
            self.ctx.audio.play(if c.fermi + c.pico == 0 {
                Sound::Bad
            } else {
                Sound::Good
            });
        }
    }

    fn record(&mut self, won: bool) {
        if self.ctx.demo {
            return;
        }
        let k = if won { "won" } else { "lost" };
        let n = self.stats.get::<u32>(k).unwrap_or(0) + 1;
        self.stats.set(k, n);
        if won {
            let total = self.stats.get::<u32>("guesses").unwrap_or(0) + self.history.len() as u32;
            self.stats.set("guesses", total);
            let best = self
                .stats
                .get::<u32>("best")
                .unwrap_or(u32::MAX)
                .min(self.history.len() as u32);
            self.stats.set("best", best);
        }
        self.stats.save(Self::ID, "scores");
    }

    /// Digits a "Bagels" clue has ruled out of the secret altogether.
    fn ruled_out(&self) -> [bool; 10] {
        let mut out = [false; 10];
        for (g, c) in &self.history {
            if c.fermi + c.pico == 0 {
                for d in g {
                    out[*d as usize] = true;
                }
            }
        }
        out
    }

    fn type_digit(&mut self, d: u8) {
        if self.phase != Phase::Guessing || self.typed.len() >= DIGITS {
            return;
        }
        if self.typed.contains(&d) {
            self.ctx.audio.play(Sound::Nope);
            self.note = "That digit is already in this guess.".into();
            return;
        }
        self.typed.push(d);
        self.ctx.audio.play(Sound::Tick);
    }
}

impl Game for Bagels {
    const ID: &'static str = "bagels";
    const TITLE: &'static str = "Bagels";
    const HELP: &'static str =
        "The computer thinks of three different digits; find them. After each
guess it tells you, without saying which digit is which:
  Fermi   a right digit in the right place
  Pico    a right digit in the wrong place
  Bagels  none of your digits is in it
Twenty guesses. Type digits, Enter to guess, Backspace to fix.
Tab shows the rules, N a new number, M sound, Esc quits.";

    fn new(ctx: Ctx) -> Self {
        let mut g = Bagels {
            ctx,
            secret: [0, 1, 2],
            history: Vec::new(),
            typed: Vec::new(),
            phase: Phase::Guessing,
            note: String::new(),
            stats: Kv::load("bagels", "scores"),
            quit: false,
            show_help: false,
            demo_clock: 0.0,
            demo_target: None,
            buttons: Vec::new(),
        };
        g.deal();
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
            Key::Tab | Key::Char('?') => self.show_help = !self.show_help,
            Key::Char('m') => {
                self.ctx.audio.toggle();
                crate::shell::remember_mute(Self::ID, self.ctx.audio.muted);
            }
            Key::Char('n') => {
                self.deal();
                self.ctx.audio.play(Sound::Click);
            }
            Key::Char(c) if c.is_ascii_digit() => self.type_digit(c as u8 - b'0'),
            Key::Backspace => {
                self.typed.pop();
            }
            Key::Enter => {
                if self.phase == Phase::Guessing {
                    self.submit();
                } else {
                    self.deal();
                }
            }
            _ => {}
        }
    }

    fn mouse(&mut self, m: Mouse) {
        if m.kind != MouseKind::Click {
            return;
        }
        if let Some((_, k)) = self.buttons.iter().find(|(r, _)| hit(r, m.x, m.y)).cloned() {
            self.key(k);
        }
    }

    fn tick(&mut self, dt: f32) {
        if !self.ctx.demo {
            return;
        }
        self.demo_clock += dt;
        let wait = if self.phase == Phase::Guessing {
            0.55
        } else {
            3.5
        };
        if self.demo_clock < wait {
            return;
        }
        self.demo_clock = 0.0;
        if self.phase != Phase::Guessing {
            self.deal();
            return;
        }
        let target = match self.demo_target {
            Some(t) => t,
            None => {
                let c = candidates(&self.history);
                let t = c[self.ctx.rng.range(0, c.len() as i64 - 1) as usize];
                self.demo_target = Some(t);
                t
            }
        };
        if self.typed.len() < DIGITS {
            let d = target[self.typed.len()];
            self.type_digit(d);
        } else {
            self.submit();
            self.demo_target = None;
        }
    }

    fn quit(&self) -> bool {
        self.quit
    }

    fn snapshot(&self) -> String {
        format!(
            "phase {:?} guesses {} typed {:?}",
            self.phase,
            self.history.len(),
            self.typed
        )
    }

    fn draw(&mut self, area: Rect, buf: &mut Buffer) {
        self.buttons.clear();
        let status = format!("guess {}/{TRIES}", (self.history.len() + 1).min(TRIES));
        let inner = frame(
            buf,
            area,
            "Bagels",
            &status,
            &[
                ("0-9", "type"),
                ("Enter", "guess"),
                ("Bksp", "fix"),
                ("Tab", "rules"),
                ("N", "new"),
                ("M", "sound"),
                ("Esc", "quit"),
            ],
        );
        if too_small(buf, inner, 60, 20) {
            return;
        }
        let left_w = (inner.width * 3 / 5).max(36);
        let left = Rect {
            width: left_w,
            ..inner
        };
        let right = Rect {
            x: inner.x + left_w,
            width: inner.width - left_w,
            ..inner
        };

        // guesses so far, the newest at the bottom, and the one being typed
        let g = panel(buf, left, "Your guesses", AMBER);
        let rows = g.height.saturating_sub(3) as usize;
        let skip = self.history.len().saturating_sub(rows);
        let mut y = g.y;
        for (i, (guess, clue)) in self.history.iter().enumerate().skip(skip) {
            put(buf, g.x, y, &format!("{:>2}", i + 1), st(DIM));
            let mut x = g.x + 4;
            for d in guess {
                x = put(buf, x, y, &format!("{d} "), bold(st(INK)));
            }
            x += 2;
            for w in clue.words() {
                let c = match w {
                    "Fermi" => GREEN,
                    "Pico" => AMBER,
                    _ => DIM,
                };
                x = put(buf, x, y, w, bold(st(c))) + 1;
            }
            y += 1;
        }
        let ty = g.bottom().saturating_sub(2);
        if self.phase == Phase::Guessing {
            put(
                buf,
                g.x,
                ty,
                &format!("{:>2}", self.history.len() + 1),
                st(AMBER),
            );
            for i in 0..DIGITS {
                let x = g.x + 4 + i as u16 * 2;
                let (s, c) = match self.typed.get(i) {
                    Some(d) => (format!("{d}"), AMBER),
                    None => ("_".into(), DIM),
                };
                put(buf, x, ty, &s, bold(st(c)));
            }
        }
        para(
            buf,
            Rect::new(g.x, g.bottom() - 1, g.width, 1),
            &self.note,
            st(match self.phase {
                Phase::Won => GREEN,
                Phase::Lost => RED,
                _ => CYAN,
            }),
        );

        // the digit pad: click to type; digits a Bagels clue ruled out are dim
        let r = panel(
            buf,
            Rect {
                height: 9.min(right.height),
                ..right
            },
            "Digits",
            AMBER,
        );
        let out = self.ruled_out();
        for d in 0..10u8 {
            let col = (d % 5) as u16;
            let row = (d / 5) as u16;
            let x = r.x + col * 5;
            let y = r.y + row * 2;
            let used = self.typed.contains(&d);
            let (fg, bg) = if used {
                (BG, AMBER)
            } else if out[d as usize] {
                (LINE, PAPER)
            } else {
                (INK, PAPER)
            };
            let cell = Rect::new(x, y, 3, 1);
            fill(buf, cell, on(fg, bg));
            put(buf, x + 1, y, &d.to_string(), bold(on(fg, bg)));
            self.buttons.push((cell, Key::Char(char::from(b'0' + d))));
        }
        let by = r.y + 4;
        if by + 1 < r.bottom() {
            put(buf, r.x, by + 1, "dim: ruled out by a Bagels", st(DIM));
        }

        // the record, or the rules
        let lower = Rect {
            y: right.y + 9,
            height: right.height.saturating_sub(9),
            ..right
        };
        if self.show_help {
            let p = panel(buf, lower, "How it works", CYAN);
            para(buf, p, "Three different digits are hidden. After each guess:\nFermi: one digit right, in the right place.\nPico: one digit right, in the wrong place.\nBagels: none of them is in the number.\nTwenty guesses; eight is plenty with a good plan.", st(INK));
        } else {
            let p = panel(buf, lower, "Your record", CYAN);
            let won = self.stats.get::<u32>("won").unwrap_or(0);
            let lost = self.stats.get::<u32>("lost").unwrap_or(0);
            let guesses = self.stats.get::<u32>("guesses").unwrap_or(0);
            let lines = [
                format!("solved     {won}"),
                format!("missed     {lost}"),
                format!(
                    "average    {}",
                    if won > 0 {
                        format!("{:.1}", guesses as f32 / won as f32)
                    } else {
                        "-".into()
                    }
                ),
                format!(
                    "best       {}",
                    self.stats
                        .get::<u32>("best")
                        .map_or("-".into(), |b| b.to_string())
                ),
                String::new(),
                format!("left       {} possible", candidates(&self.history).len()),
            ];
            for (i, l) in lines.iter().enumerate() {
                if p.y + (i as u16) < p.bottom() {
                    put(
                        buf,
                        p.x,
                        p.y + i as u16,
                        l,
                        st(if i == 5 { DIM } else { INK }),
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scoring() {
        assert_eq!(score(&[1, 2, 3], &[1, 2, 3]), Clue { fermi: 3, pico: 0 });
        assert_eq!(score(&[1, 2, 3], &[3, 1, 2]), Clue { fermi: 0, pico: 3 });
        assert_eq!(score(&[1, 2, 3], &[1, 5, 2]), Clue { fermi: 1, pico: 1 });
        assert_eq!(score(&[1, 2, 3], &[4, 5, 6]).words(), vec!["Bagels"]);
    }

    #[test]
    fn there_are_720_secrets_and_clues_narrow_them() {
        assert_eq!(all().len(), 720);
        let s = [4, 0, 7];
        let mut h = Vec::new();
        for g in [[1, 2, 3], [4, 5, 6], [7, 8, 9], [0, 4, 7]] {
            h.push((g, score(&s, &g)));
        }
        let c = candidates(&h);
        assert!(c.contains(&s));
        assert!(c.len() < 10);
    }
}
