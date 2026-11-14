//! Hide and Seek: three ways to find something hidden on a grid from the
//! clues each guess brings back. Hurkle (which way), Mugwump (how far) and
//! Depth Charge (a submarine in a cube of sea). Rules in
//! asciiverse/specs/hide-and-seek.md.

use crate::audio::Sound;
use crate::rng::Rng;
use crate::shell::{Ctx, Game, Key, Mouse, MouseKind};
use crate::store::Kv;
use crate::ui::*;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

pub const SIZE: i32 = 10;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Variant {
    Hurkle,
    Mugwump,
    Depth,
}

pub const VARIANTS: [Variant; 3] = [Variant::Hurkle, Variant::Mugwump, Variant::Depth];

impl Variant {
    pub fn name(self) -> &'static str {
        match self {
            Variant::Hurkle => "Hurkle",
            Variant::Mugwump => "Mugwump",
            Variant::Depth => "Depth Charge",
        }
    }
    fn key(self) -> &'static str {
        match self {
            Variant::Hurkle => "hurkle",
            Variant::Mugwump => "mugwump",
            Variant::Depth => "depth",
        }
    }
    pub fn guesses(self) -> usize {
        match self {
            Variant::Hurkle => 5,
            Variant::Mugwump => 10,
            Variant::Depth => 5,
        }
    }
    fn blurb(self) -> &'static str {
        match self {
            Variant::Hurkle => "One shy hurkle hides on a ten by ten field. Each guess, it tells you which way to look: north, south-west and so on. Five guesses.",
            Variant::Mugwump => "Four mugwumps hide on the field. Each guess, every one still hidden tells you how far away it is. Ten guesses to find all four.",
            Variant::Depth => "A submarine lies somewhere in a cube of sea, ten by ten and ten deep. Each depth charge says which way it is, and whether it is deeper or shallower. Five charges.",
        }
    }
}

/// A place on the field: x east from the west edge, y north from the south
/// edge, z down from the surface (Depth Charge only).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pos {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

/// Which way the target lies from a guess, as a compass word, or "" when
/// the guess is on it (x and y only).
pub fn compass(from: Pos, to: Pos) -> &'static str {
    let ns = (to.y - from.y).signum();
    let ew = (to.x - from.x).signum();
    match (ns, ew) {
        (1, 0) => "north",
        (1, 1) => "north-east",
        (0, 1) => "east",
        (-1, 1) => "south-east",
        (-1, 0) => "south",
        (-1, -1) => "south-west",
        (0, -1) => "west",
        (1, -1) => "north-west",
        _ => "",
    }
}

fn arrow(word: &str) -> &'static str {
    match word {
        "north" => "↑",
        "north-east" => "↗",
        "east" => "→",
        "south-east" => "↘",
        "south" => "↓",
        "south-west" => "↙",
        "west" => "←",
        "north-west" => "↖",
        _ => "●",
    }
}

/// Straight-line distance across the field, to a tenth.
pub fn distance(a: Pos, b: Pos) -> f32 {
    let d = (((a.x - b.x).pow(2) + (a.y - b.y).pow(2)) as f32).sqrt();
    (d * 10.0).round() / 10.0
}

/// What a guess brought back.
#[derive(Clone, Debug, PartialEq)]
pub enum Clue {
    Dir(&'static str),
    Dist(Vec<f32>),
    Sonar { dir: &'static str, depth: i32 },
}

#[derive(Clone, Debug)]
pub struct Guess {
    pub at: Pos,
    pub clue: Clue,
    /// mugwumps found by this guess
    pub found: usize,
}

/// One round of any of the three.
#[derive(Clone, Debug)]
pub struct Hunt {
    pub variant: Variant,
    pub hidden: Vec<Pos>,
    pub found: Vec<bool>,
    pub guesses: Vec<Guess>,
}

impl Hunt {
    pub fn new(variant: Variant, rng: &mut Rng) -> Hunt {
        let n = if variant == Variant::Mugwump { 4 } else { 1 };
        let mut hidden: Vec<Pos> = Vec::new();
        while hidden.len() < n {
            let p = Pos {
                x: rng.range(0, SIZE as i64 - 1) as i32,
                y: rng.range(0, SIZE as i64 - 1) as i32,
                z: if variant == Variant::Depth {
                    rng.range(0, SIZE as i64 - 1) as i32
                } else {
                    0
                },
            };
            if !hidden.contains(&p) {
                hidden.push(p);
            }
        }
        Hunt {
            variant,
            found: vec![false; n],
            hidden,
            guesses: Vec::new(),
        }
    }

    pub fn won(&self) -> bool {
        self.found.iter().all(|&f| f)
    }

    pub fn over(&self) -> bool {
        self.won() || self.guesses.len() >= self.variant.guesses()
    }

    pub fn left(&self) -> usize {
        self.variant.guesses() - self.guesses.len()
    }

    /// Make a guess and return what it found out.
    pub fn guess(&mut self, at: Pos) -> &Guess {
        let mut found = 0;
        for (i, h) in self.hidden.iter().enumerate() {
            if !self.found[i] && h.x == at.x && h.y == at.y && h.z == at.z {
                self.found[i] = true;
                found += 1;
            }
        }
        let t = self.hidden[0];
        let clue = match self.variant {
            Variant::Hurkle => Clue::Dir(compass(at, t)),
            Variant::Mugwump => Clue::Dist(
                self.hidden
                    .iter()
                    .zip(&self.found)
                    .filter(|(_, f)| !**f)
                    .map(|(h, _)| distance(at, *h))
                    .collect(),
            ),
            Variant::Depth => Clue::Sonar {
                dir: compass(at, t),
                depth: (t.z - at.z).signum(),
            },
        };
        self.guesses.push(Guess { at, clue, found });
        self.guesses.last().unwrap()
    }

    /// Could something still be hiding at `p`, given every clue so far?
    pub fn possible(&self, p: Pos) -> bool {
        self.guesses.iter().all(|g| match &g.clue {
            Clue::Dir(d) => compass(g.at, p) == *d,
            Clue::Sonar { dir, depth } => {
                compass(g.at, p) == *dir && (p.z - g.at.z).signum() == *depth
            }
            Clue::Dist(ds) => {
                // a guess that found one says nothing about where the others are
                // beyond their distances; one of them must match
                if g.at.x == p.x && g.at.y == p.y {
                    return false;
                }
                ds.iter().any(|d| (distance(g.at, p) - d).abs() < 0.05)
            }
        })
    }
}

/// The demo's guess: the middle of what is still possible (Hurkle, Depth
/// Charge), or a cell that fits every clue (Mugwump).
pub fn hint(h: &Hunt, rng: &mut Rng) -> Pos {
    let zs: Vec<i32> = if h.variant == Variant::Depth {
        (0..SIZE).collect()
    } else {
        vec![0]
    };
    let mut cand = Vec::new();
    for &z in &zs {
        for y in 0..SIZE {
            for x in 0..SIZE {
                let p = Pos { x, y, z };
                if h.possible(p) && !h.guesses.iter().any(|g| g.at == p) {
                    cand.push(p);
                }
            }
        }
    }
    if cand.is_empty() {
        return Pos { x: 4, y: 4, z: 4 };
    }
    if h.variant == Variant::Mugwump {
        return cand[rng.range(0, cand.len() as i64 - 1) as usize];
    }
    let mid = |f: &dyn Fn(&Pos) -> i32| {
        let lo = cand.iter().map(f).min().unwrap();
        let hi = cand.iter().map(f).max().unwrap();
        (lo + hi) / 2
    };
    Pos {
        x: mid(&|p| p.x),
        y: mid(&|p| p.y),
        z: mid(&|p| p.z),
    }
}

fn plural(n: usize) -> String {
    if n == 1 {
        "1 guess".into()
    } else {
        format!("{n} guesses")
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Phase {
    Menu,
    Playing,
    Over,
}

pub struct HideSeek {
    ctx: Ctx,
    phase: Phase,
    sel: usize,
    hunt: Hunt,
    cursor: Pos,
    shade: bool,
    note: String,
    score: Kv,
    show_help: bool,
    quit: bool,
    cells: Vec<(Rect, i32, i32)>,
    depths: Vec<(Rect, i32)>,
    rows: Vec<Rect>,
    demo_clock: f32,
    demo_round: usize,
    demo_target: Option<Pos>,
}

impl HideSeek {
    fn start(&mut self, v: Variant) {
        self.hunt = Hunt::new(v, &mut self.ctx.rng);
        self.phase = Phase::Playing;
        self.cursor = Pos { x: 4, y: 4, z: 4 };
        self.note = match v {
            Variant::Hurkle => "Where is the hurkle? Pick a spot and press Enter.",
            Variant::Mugwump => "Four mugwumps are hiding. Pick a spot and press Enter.",
            Variant::Depth => "Set the depth with PgUp and PgDn, aim, and drop a charge.",
        }
        .into();
        log::info!("start {}", v.name());
    }

    fn fire(&mut self) {
        if self.phase != Phase::Playing {
            return;
        }
        let at = self.cursor;
        let at = if self.hunt.variant == Variant::Depth {
            at
        } else {
            Pos { z: 0, ..at }
        };
        if self.hunt.guesses.iter().any(|g| g.at == at) {
            self.note = "You have tried that spot already.".into();
            self.ctx.audio.play(Sound::Nope);
            return;
        }
        let v = self.hunt.variant;
        let g = self.hunt.guess(at).clone();
        log::info!("guess {:?} -> {:?}", g.at, g.clue);
        if v == Variant::Depth {
            self.ctx.audio.play(Sound::Boom);
        }
        if self.hunt.won() {
            self.phase = Phase::Over;
            self.note = match v {
                Variant::Hurkle => format!(
                    "Found it in {}! The hurkle blinks at you and scuttles off. Enter to play again.",
                    self.hunt.guesses.len()
                ),
                Variant::Mugwump => format!(
                    "All four mugwumps found in {}! Enter to play again.",
                    self.hunt.guesses.len()
                ),
                Variant::Depth => format!(
                    "A hit in {}! The submarine surfaces and its crew wave a white flag. Enter to play again.",
                    self.hunt.guesses.len()
                ),
            };
            self.ctx.audio.play(Sound::Win);
            self.record(true);
            return;
        }
        if self.hunt.over() {
            self.phase = Phase::Over;
            self.note = match v {
                Variant::Hurkle => "Out of guesses. The hurkle was here (H). Enter to try again.",
                Variant::Mugwump => "Out of guesses. The rest were here (M). Enter to try again.",
                Variant::Depth => {
                    "Out of charges. The submarine slips away (◆). Enter to try again."
                }
            }
            .into();
            self.ctx.audio.play(Sound::Lose);
            self.record(false);
            return;
        }
        self.note = match &g.clue {
            Clue::Dir(d) => format!("Go {d}."),
            Clue::Dist(ds) => {
                let list: Vec<String> = ds.iter().map(|d| format!("{d:.1}")).collect();
                if g.found > 0 {
                    format!("You found one! The others: {}.", list.join(", "))
                } else {
                    format!("Distances: {}.", list.join(", "))
                }
            }
            Clue::Sonar { dir, depth } => {
                let z = match depth {
                    1 => "deeper",
                    -1 => "shallower",
                    _ => "at this depth",
                };
                if dir.is_empty() {
                    format!("Right above or below it: it is {z}.")
                } else {
                    format!("Sonar: {dir}, and {z}.")
                }
            }
        };
        self.ctx.audio.play(if g.found > 0 {
            Sound::Good
        } else {
            Sound::Click
        });
    }

    fn record(&mut self, won: bool) {
        if self.ctx.demo {
            return;
        }
        let v = self.hunt.variant.key();
        let k = format!("{v}_{}", if won { "won" } else { "lost" });
        let n = self.score.get::<u32>(&k).unwrap_or(0) + 1;
        self.score.set(&k, n);
        if won {
            let best = self
                .score
                .get::<u32>(&format!("{v}_best"))
                .unwrap_or(u32::MAX)
                .min(self.hunt.guesses.len() as u32);
            self.score.set(&format!("{v}_best"), best);
        }
        self.score.save(Self::ID, "score");
    }

    fn step(&mut self, dx: i32, dy: i32, dz: i32) {
        let c = |v: i32| v.clamp(0, SIZE - 1);
        self.cursor = Pos {
            x: c(self.cursor.x + dx),
            y: c(self.cursor.y + dy),
            z: c(self.cursor.z + dz),
        };
        self.ctx.audio.play(Sound::Tick);
    }

    fn draw_menu(&mut self, buf: &mut Buffer, inner: Rect) {
        let w = 64.min(inner.width - 2);
        let x = inner.x + (inner.width - w) / 2;
        let mut y = inner.y + 1;
        centre(
            buf,
            inner,
            y,
            "Something is hiding. Which game?",
            bold(st(AMBER)),
        );
        y += 2;
        for (i, v) in VARIANTS.iter().enumerate() {
            let h = 6;
            let r = Rect::new(x, y, w, h);
            let p = panel(
                buf,
                r,
                &format!("{} {}", i + 1, v.name()),
                if i == self.sel { AMBER } else { DIM },
            );
            if i == self.sel {
                put(buf, x.saturating_sub(2), y + 2, "▸", bold(st(AMBER)));
            }
            para(buf, p, v.blurb(), st(if i == self.sel { INK } else { DIM }));
            let k = v.key();
            let won = self.score.get::<u32>(&format!("{k}_won")).unwrap_or(0);
            let lost = self.score.get::<u32>(&format!("{k}_lost")).unwrap_or(0);
            let best = self
                .score
                .get::<u32>(&format!("{k}_best"))
                .map_or("-".into(), |b| b.to_string());
            if p.height >= 4 {
                put(
                    buf,
                    p.x,
                    p.y + 3,
                    &format!("found {won}   missed {lost}   best {best}"),
                    st(CYAN),
                );
            }
            self.rows.push(r);
            y += h;
            if y + h > inner.bottom() {
                break;
            }
        }
    }

    fn draw_field(&mut self, buf: &mut Buffer, inner: Rect) {
        let v = self.hunt.variant;
        let tall = inner.height >= 26;
        let ch: u16 = if tall { 2 } else { 1 };
        let gx = inner.x + 4;
        let gy = inner.y + 1;
        let over = self.phase == Phase::Over;
        let water = Color::Rgb(14, 26, 40);
        let grass = Color::Rgb(16, 30, 18);
        let ground = if v == Variant::Depth { water } else { grass };
        for y in 0..SIZE {
            let row = SIZE - 1 - y;
            let sy = gy + row as u16 * ch;
            put(buf, inner.x + 1, sy + ch / 2, &format!("{y:>2}"), st(DIM));
            for x in 0..SIZE {
                let sx = gx + x as u16 * 4;
                let r = Rect::new(sx, sy, 4, ch);
                let p2 = Pos {
                    x,
                    y,
                    z: self.cursor.z,
                };
                let possible = self.shade
                    && match v {
                        Variant::Depth => (0..SIZE).any(|z| self.hunt.possible(Pos { z, ..p2 })),
                        _ => self.hunt.possible(Pos { z: 0, ..p2 }),
                    };
                let cursor = self.cursor.x == x && self.cursor.y == y && !over;
                let bg = if cursor {
                    Color::Rgb(90, 70, 20)
                } else if possible {
                    mix(ground, AMBER, 0.18)
                } else {
                    ground
                };
                fill(buf, r, on(DIM, bg));
                // what is shown in the cell: the newest guess there, a find, the reveal
                let mut text = String::from("·");
                let mut fg = mix(bg, INK, 0.35);
                for g in &self.hunt.guesses {
                    if g.at.x == x && g.at.y == y {
                        match &g.clue {
                            Clue::Dir(d) => {
                                text = arrow(d).into();
                                fg = AMBER;
                            }
                            Clue::Dist(ds) => {
                                text = ds
                                    .iter()
                                    .cloned()
                                    .fold(None, |m: Option<f32>, d| Some(m.map_or(d, |m| m.min(d))))
                                    .map_or("·".into(), |d| format!("{d:.1}"));
                                fg = AMBER;
                            }
                            Clue::Sonar { dir, depth } => {
                                text = format!(
                                    "{}{}",
                                    arrow(dir),
                                    match depth {
                                        1 => "▼",
                                        -1 => "▲",
                                        _ => "=",
                                    }
                                );
                                fg = AMBER;
                            }
                        }
                        if g.found > 0 {
                            text = if v == Variant::Mugwump { "M!" } else { "★" }.into();
                            fg = GREEN;
                        }
                    }
                }
                if over {
                    for (i, h) in self.hunt.hidden.iter().enumerate() {
                        if h.x == x && h.y == y && !self.hunt.found[i] {
                            text = match v {
                                Variant::Hurkle => "H",
                                Variant::Mugwump => "M",
                                Variant::Depth => "◆",
                            }
                            .into();
                            fg = RED;
                        }
                    }
                }
                let w = text.chars().count() as u16;
                put(
                    buf,
                    sx + (4 - w.min(4)) / 2,
                    sy + ch / 2,
                    &text,
                    bold(on(fg, bg)),
                );
                self.cells.push((r, x, y));
            }
        }
        let by = gy + SIZE as u16 * ch;
        for x in 0..SIZE {
            put(buf, gx + x as u16 * 4 + 1, by, &format!("{x}"), st(DIM));
        }
        put(buf, gx + 40, gy, "N", bold(st(DIM)));
        put(buf, gx + 40, by.saturating_sub(1), "S", bold(st(DIM)));

        // Depth Charge's depth gauge
        if v == Variant::Depth {
            let dx = gx + 43;
            put(buf, dx, gy.saturating_sub(1).max(inner.y), "depth", st(DIM));
            for z in 0..SIZE {
                let sy = gy + z as u16 * ch;
                let r = Rect::new(dx, sy, 5, ch);
                let sel = z == self.cursor.z;
                let bg = if sel {
                    Color::Rgb(90, 70, 20)
                } else {
                    mix(water, BG, z as f32 / SIZE as f32 * 0.6)
                };
                fill(buf, r, on(INK, bg));
                let mut label = format!("{z:>2}");
                if over && self.hunt.hidden[0].z == z {
                    label.push_str(" ◆");
                }
                put(
                    buf,
                    dx,
                    sy + ch / 2,
                    &label,
                    bold(on(if sel { AMBER } else { INK }, bg)),
                );
                self.depths.push((r, z));
            }
        }
    }
}

impl Game for HideSeek {
    const ID: &'static str = "hide-and-seek";
    const TITLE: &'static str = "Hide and Seek";
    const HELP: &'static str =
        "Three games of finding something hidden from the clues each guess brings:
  Hurkle        one hurkle on a 10x10 field; each guess says which way
  Mugwump       four mugwumps; each guess says how far each one is
  Depth Charge  a submarine in a 10x10x10 sea; each charge says which way
                and whether it is deeper or shallower
Arrows aim (PgUp and PgDn set the depth), Enter guesses, or click the
field. H shades every spot that still fits the clues. V goes back to the
choice of game, Tab shows the rules.";

    fn new(ctx: Ctx) -> Self {
        let score = if ctx.demo {
            Kv::default()
        } else {
            Kv::load(Self::ID, "score")
        };
        let mut ctx = ctx;
        let hunt = Hunt::new(Variant::Hurkle, &mut ctx.rng);
        let mut g = HideSeek {
            phase: Phase::Menu,
            sel: 0,
            hunt,
            cursor: Pos { x: 4, y: 4, z: 4 },
            shade: false,
            note: String::new(),
            score,
            show_help: false,
            quit: false,
            cells: Vec::new(),
            depths: Vec::new(),
            rows: Vec::new(),
            demo_clock: 0.0,
            demo_round: 0,
            demo_target: None,
            ctx,
        };
        if g.ctx.demo {
            g.shade = true;
            g.start(Variant::Hurkle);
        }
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
            Key::Esc | Key::Char('q') => {
                if self.phase == Phase::Menu {
                    self.quit = true;
                } else {
                    self.phase = Phase::Menu;
                }
            }
            Key::Tab | Key::Char('?') => self.show_help = !self.show_help,
            Key::Char('m') => {
                self.ctx.audio.toggle();
                crate::shell::remember_mute(Self::ID, self.ctx.audio.muted);
            }
            _ => {}
        }
        match self.phase {
            Phase::Menu => match k {
                Key::Up => self.sel = (self.sel + 2) % 3,
                Key::Down => self.sel = (self.sel + 1) % 3,
                Key::Char(c @ '1'..='3') => {
                    self.sel = c as usize - '1' as usize;
                    self.start(VARIANTS[self.sel]);
                }
                Key::Enter | Key::Char(' ') => self.start(VARIANTS[self.sel]),
                _ => {}
            },
            Phase::Playing => match k {
                Key::Up => self.step(0, 1, 0),
                Key::Down => self.step(0, -1, 0),
                Key::Left => self.step(-1, 0, 0),
                Key::Right => self.step(1, 0, 0),
                Key::PageUp | Key::Char('[') => self.step(0, 0, -1),
                Key::PageDown | Key::Char(']') => self.step(0, 0, 1),
                Key::Enter | Key::Char(' ') => self.fire(),
                Key::Char('h') => self.shade = !self.shade,
                Key::Char('v') => self.phase = Phase::Menu,
                _ => {}
            },
            Phase::Over => match k {
                Key::Enter | Key::Char(' ') | Key::Char('n') => self.start(self.hunt.variant),
                Key::Char('v') => self.phase = Phase::Menu,
                Key::Char('h') => self.shade = !self.shade,
                _ => {}
            },
        }
    }

    fn mouse(&mut self, m: Mouse) {
        if self.ctx.demo {
            return;
        }
        match self.phase {
            Phase::Menu => {
                if let Some(i) = self.rows.iter().position(|r| hit(r, m.x, m.y)) {
                    self.sel = i;
                    if m.kind == MouseKind::Click {
                        self.start(VARIANTS[i]);
                    }
                }
            }
            Phase::Playing => {
                if m.kind == MouseKind::ScrollUp {
                    self.step(0, 0, -1);
                } else if m.kind == MouseKind::ScrollDown {
                    self.step(0, 0, 1);
                }
                if let Some((_, x, y)) = self.cells.iter().find(|(r, _, _)| hit(r, m.x, m.y)) {
                    self.cursor.x = *x;
                    self.cursor.y = *y;
                    if m.kind == MouseKind::Click {
                        self.fire();
                    }
                } else if let Some((_, z)) = self.depths.iter().find(|(r, _)| hit(r, m.x, m.y)) {
                    if m.kind == MouseKind::Click {
                        self.cursor.z = *z;
                        self.ctx.audio.play(Sound::Tick);
                    }
                }
            }
            Phase::Over => {
                if m.kind == MouseKind::Click {
                    self.start(self.hunt.variant);
                }
            }
        }
    }

    fn tick(&mut self, dt: f32) {
        if !self.ctx.demo {
            return;
        }
        self.demo_clock += dt;
        match self.phase {
            Phase::Over => {
                if self.demo_clock > 3.5 {
                    self.demo_clock = 0.0;
                    self.demo_round += 1;
                    self.start(VARIANTS[self.demo_round % 3]);
                }
            }
            _ => {
                if self.demo_clock < 0.35 {
                    return;
                }
                self.demo_clock = 0.0;
                let t = match self.demo_target {
                    Some(t) => t,
                    None => {
                        let t = hint(&self.hunt, &mut self.ctx.rng);
                        self.demo_target = Some(t);
                        t
                    }
                };
                let c = self.cursor;
                // walk the cursor there a step at a time, then fire
                if c.x != t.x || c.y != t.y || (self.hunt.variant == Variant::Depth && c.z != t.z) {
                    self.cursor = Pos {
                        x: c.x + (t.x - c.x).signum(),
                        y: c.y + (t.y - c.y).signum(),
                        z: c.z + (t.z - c.z).signum(),
                    };
                } else {
                    self.demo_target = None;
                    self.fire();
                }
            }
        }
    }

    fn quit(&self) -> bool {
        self.quit
    }

    fn snapshot(&self) -> String {
        format!(
            "phase {:?} variant {:?} guesses {} cursor {:?}",
            self.phase,
            self.hunt.variant,
            self.hunt.guesses.len(),
            self.cursor
        )
    }

    fn draw(&mut self, area: Rect, buf: &mut Buffer) {
        self.cells.clear();
        self.depths.clear();
        self.rows.clear();
        let menu = self.phase == Phase::Menu;
        let status = if menu {
            String::new()
        } else {
            format!("{}: {} left", self.hunt.variant.name(), self.hunt.left())
        };
        let keys: &[(&str, &str)] = if menu {
            &[
                ("↑↓", "choose"),
                ("Enter", "play"),
                ("Tab", "rules"),
                ("M", "sound"),
                ("Esc", "quit"),
            ]
        } else {
            &[
                ("←↑↓→", "aim"),
                ("PgUp/Dn", "depth"),
                ("Enter", "guess"),
                ("H", "shade"),
                ("V", "games"),
                ("Tab", "rules"),
                ("M", "sound"),
                ("Esc", "back"),
            ]
        };
        let inner = frame(buf, area, Self::TITLE, &status, keys);
        if too_small(buf, inner, 78, 22) {
            return;
        }
        if menu {
            if self.show_help {
                let p = panel(buf, inner, "How it works", CYAN);
                para(buf, p, Self::HELP, st(INK));
            } else {
                self.draw_menu(buf, inner);
            }
            return;
        }
        self.draw_field(buf, inner);

        let v = self.hunt.variant;
        let rx = inner.x + if v == Variant::Depth { 54 } else { 48 };
        let right = Rect::new(
            rx,
            inner.y,
            inner.right().saturating_sub(rx + 1),
            inner.height,
        );
        let top_h = 9.min(right.height);
        let p = panel(
            buf,
            Rect {
                height: top_h,
                ..right
            },
            v.name(),
            AMBER,
        );
        let a = self.cursor;
        let aim = if v == Variant::Depth {
            format!("aim: {},{}  depth {}", a.x, a.y, a.z)
        } else {
            format!("aim: {},{}", a.x, a.y)
        };
        put(buf, p.x, p.y, &aim, st(DIM));
        let found = self.hunt.found.iter().filter(|f| **f).count();
        let line = match v {
            Variant::Mugwump => format!("{} left, {found} of 4 found", plural(self.hunt.left())),
            _ => format!("{} left", plural(self.hunt.left())),
        };
        put(buf, p.x, p.y + 1, &line, st(INK));
        para(
            buf,
            Rect::new(p.x, p.y + 3, p.width, p.height.saturating_sub(3)),
            &self.note,
            st(match (self.phase, self.hunt.won()) {
                (Phase::Over, true) => GREEN,
                (Phase::Over, false) => RED,
                _ => CYAN,
            }),
        );

        let lower = Rect {
            y: right.y + top_h,
            height: right.height.saturating_sub(top_h),
            ..right
        };
        if self.show_help {
            let p = panel(buf, lower, "How it works", CYAN);
            para(buf, p, v.blurb(), st(INK));
            let y = p.y + 5;
            if y < p.bottom() {
                para(
                    buf,
                    Rect::new(p.x, y, p.width, p.bottom() - y),
                    "Rows count north from 0 at the bottom, columns east from 0 at the left. H shades every spot that still fits the clues.",
                    st(DIM),
                );
            }
        } else {
            let p = panel(buf, lower, "Clues", CYAN);
            let rows = p.height as usize;
            let skip = self.hunt.guesses.len().saturating_sub(rows);
            for (i, g) in self.hunt.guesses.iter().enumerate().skip(skip) {
                let y = p.y + (i - skip) as u16;
                let at = if v == Variant::Depth {
                    format!("{},{},{}", g.at.x, g.at.y, g.at.z)
                } else {
                    format!("{},{}", g.at.x, g.at.y)
                };
                let clue = match &g.clue {
                    _ if g.found > 0 && v != Variant::Mugwump => "found!".to_string(),
                    Clue::Dir(d) => format!("go {d}"),
                    Clue::Dist(ds) => {
                        let l: Vec<String> = ds.iter().map(|d| format!("{d:.1}")).collect();
                        if g.found > 0 && l.is_empty() {
                            "found the last one".to_string()
                        } else {
                            let f = if g.found > 0 { "found one; " } else { "" };
                            format!("{f}{}", l.join(" "))
                        }
                    }
                    Clue::Sonar { dir, depth } => format!(
                        "{} {}",
                        if dir.is_empty() { "here" } else { dir },
                        match depth {
                            1 => "deeper",
                            -1 => "shallower",
                            _ => "this depth",
                        }
                    ),
                };
                put_max(
                    buf,
                    p.x,
                    y,
                    &format!("{:>2}  {at:<8} {clue}", i + 1),
                    st(if g.found > 0 { GREEN } else { INK }),
                    p.width,
                );
            }
            if self.hunt.guesses.is_empty() {
                put_max(buf, p.x, p.y, "No guesses yet.", st(DIM), p.width);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(x: i32, y: i32) -> Pos {
        Pos { x, y, z: 0 }
    }

    #[test]
    fn compass_words() {
        assert_eq!(compass(at(0, 0), at(0, 5)), "north");
        assert_eq!(compass(at(5, 5), at(2, 1)), "south-west");
        assert_eq!(compass(at(3, 3), at(3, 3)), "");
    }

    #[test]
    fn hurkle_can_always_be_found_in_five() {
        for seed in 1..60 {
            let mut rng = Rng::new(seed);
            let mut h = Hunt::new(Variant::Hurkle, &mut rng);
            while !h.over() {
                let p = hint(&h, &mut rng);
                h.guess(p);
            }
            assert!(h.won(), "seed {seed}");
        }
    }

    #[test]
    fn depth_charge_halves_three_ways() {
        let mut rng = Rng::new(5);
        let mut wins = 0;
        for _ in 0..40 {
            let mut h = Hunt::new(Variant::Depth, &mut rng);
            while !h.over() {
                let p = hint(&h, &mut rng);
                h.guess(p);
            }
            wins += h.won() as u32;
        }
        assert!(wins >= 30, "{wins}");
    }

    #[test]
    fn mugwump_clues_keep_the_hidden_ones_possible() {
        let mut rng = Rng::new(9);
        let mut h = Hunt::new(Variant::Mugwump, &mut rng);
        h.guess(at(0, 0));
        h.guess(at(9, 9));
        for (i, p) in h.hidden.clone().iter().enumerate() {
            if !h.found[i] {
                assert!(h.possible(*p));
            }
        }
        assert_eq!(distance(at(0, 0), at(3, 4)), 5.0);
    }
}
