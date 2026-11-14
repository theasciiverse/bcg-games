//! 3-D Tic-Tac-Toe: four in a row in a 4x4x4 cube, against the computer.
//! Rules in asciiverse/specs/3d-tic-tac-toe.md.

use crate::audio::Sound;
use crate::rng::Rng;
use crate::shell::{Ctx, Game, Key, Mouse, MouseKind};
use crate::store::Kv;
use crate::ui::*;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

pub const N: usize = 4;
pub const CELLS: usize = N * N * N;

/// 0 empty, 1 the player (X), 2 the computer (O). Index level * 16 + row *
/// 4 + column, level 0 at the top of the cube.
pub type Board = [u8; CELLS];

pub fn idx(level: usize, row: usize, col: usize) -> usize {
    level * N * N + row * N + col
}

pub fn coords(i: usize) -> (usize, usize, usize) {
    (i / (N * N), (i / N) % N, i % N)
}

/// Every line of four through the cube: 76 of them (48 straight along an
/// axis, 24 diagonals across a face or a slice, 4 through the centre).
pub fn lines() -> Vec<[usize; N]> {
    let mut out = Vec::new();
    for dl in -1i32..=1 {
        for dr in -1i32..=1 {
            for dc in -1i32..=1 {
                if (dl, dr, dc) <= (0, 0, 0) {
                    // each direction once, not also its opposite
                    continue;
                }
                for start in 0..CELLS {
                    let (l, r, c) = coords(start);
                    let (l, r, c) = (l as i32, r as i32, c as i32);
                    let inside = |a: i32| (0..N as i32).contains(&a);
                    // a line starts where one step back leaves the cube
                    if inside(l - dl) && inside(r - dr) && inside(c - dc) {
                        continue;
                    }
                    let mut cells = [0; N];
                    let mut ok = true;
                    for (k, cell) in cells.iter_mut().enumerate() {
                        let k = k as i32;
                        let (a, b, d) = (l + dl * k, r + dr * k, c + dc * k);
                        if !(inside(a) && inside(b) && inside(d)) {
                            ok = false;
                            break;
                        }
                        *cell = idx(a as usize, b as usize, d as usize);
                    }
                    if ok {
                        out.push(cells);
                    }
                }
            }
        }
    }
    out
}

/// The winning line, if a side has one.
pub fn winner(b: &Board, ls: &[[usize; N]]) -> Option<(u8, [usize; N])> {
    for l in ls {
        let v = b[l[0]];
        if v != 0 && l.iter().all(|&i| b[i] == v) {
            return Some((v, *l));
        }
    }
    None
}

/// Cells that would complete a line of four for `side` right now.
pub fn threats(b: &Board, side: u8, ls: &[[usize; N]]) -> Vec<usize> {
    let mut out = Vec::new();
    for l in ls {
        let mine = l.iter().filter(|&&i| b[i] == side).count();
        let empty: Vec<usize> = l.iter().copied().filter(|&i| b[i] == 0).collect();
        if mine == 3 && empty.len() == 1 && !out.contains(&empty[0]) {
            out.push(empty[0]);
        }
    }
    out
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Level {
    Easy,
    Fair,
    Hard,
}

impl Level {
    fn name(self) -> &'static str {
        match self {
            Level::Easy => "easy",
            Level::Fair => "fair",
            Level::Hard => "hard",
        }
    }
}

/// How much a cell is worth to `side`: open lines through it, weighted by
/// how many of its marks they already hold, ours and the opponent's.
fn worth(b: &Board, cell: usize, side: u8, ls: &[[usize; N]]) -> f32 {
    let other = 3 - side;
    let mut s = 0.0;
    for l in ls.iter().filter(|l| l.contains(&cell)) {
        let mine = l.iter().filter(|&&i| b[i] == side).count();
        let theirs = l.iter().filter(|&&i| b[i] == other).count();
        s += match (mine, theirs) {
            (_, 0) => [1.0, 5.0, 40.0, 0.0][mine],
            (0, _) => [0.0, 4.0, 30.0, 0.0][theirs],
            _ => 0.0,
        };
    }
    s
}

/// The computer's move for `side`: win if it can, block if it must, make
/// two threats at once if it can, stop the opponent doing the same, else the
/// best-placed cell, with more noise the easier the level.
pub fn choose(b: &Board, side: u8, level: Level, rng: &mut Rng, ls: &[[usize; N]]) -> usize {
    let other = 3 - side;
    let empty: Vec<usize> = (0..CELLS).filter(|&i| b[i] == 0).collect();
    if let Some(&w) = threats(b, side, ls).first() {
        return w;
    }
    let blocks = threats(b, other, ls);
    if !blocks.is_empty() && (level != Level::Easy || rng.chance(0.85)) {
        return blocks[0];
    }
    if level != Level::Easy {
        // a fork: one move that leaves two ways to win
        for &c in &empty {
            let mut n = *b;
            n[c] = side;
            if threats(&n, side, ls).len() >= 2 {
                return c;
            }
        }
        // the opponent's fork, taken first
        let mut forks = Vec::new();
        for &c in &empty {
            let mut n = *b;
            n[c] = other;
            if threats(&n, other, ls).len() >= 2 {
                forks.push(c);
            }
        }
        if level == Level::Hard && !forks.is_empty() {
            // better still: a threat of our own that makes them answer
            for &c in &empty {
                let mut n = *b;
                n[c] = side;
                let t = threats(&n, side, ls);
                if t.len() == 1 && forks.contains(&t[0]) {
                    return c;
                }
            }
        }
        if let Some(&f) = forks.first() {
            return f;
        }
    }
    let noise = match level {
        Level::Easy => 30.0,
        Level::Fair => 6.0,
        Level::Hard => 0.5,
    };
    let mut best = empty[0];
    let mut best_s = f32::MIN;
    for &c in &empty {
        let s = worth(b, c, side, ls) + rng.unit() * noise;
        if s > best_s {
            best_s = s;
            best = c;
        }
    }
    best
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Turn {
    You,
    Computer,
    Over(u8),
}

pub struct Qubic {
    ctx: Ctx,
    lines: Vec<[usize; N]>,
    board: Board,
    turn: Turn,
    /// who moved first this game, alternating
    first: u8,
    level: Level,
    cursor: usize,
    last: Option<usize>,
    win_line: Option<[usize; N]>,
    think: f32,
    note: String,
    score: Kv,
    show_help: bool,
    quit: bool,
    cells: Vec<(Rect, usize)>,
    buttons: Vec<(Rect, Key)>,
    demo_clock: f32,
    moves: u32,
}

impl Qubic {
    fn new_game(&mut self) {
        self.board = [0; CELLS];
        self.first = 3 - self.first;
        self.turn = if self.first == 1 {
            Turn::You
        } else {
            Turn::Computer
        };
        self.last = None;
        self.win_line = None;
        self.think = 0.0;
        self.moves = 0;
        self.note = if self.first == 1 {
            "You move first. Four in a row, in any direction through the cube.".into()
        } else {
            "The computer moves first this time.".into()
        };
        log::info!("new game, first {} level {:?}", self.first, self.level);
    }

    fn place(&mut self, cell: usize, side: u8) {
        self.board[cell] = side;
        self.last = Some(cell);
        self.moves += 1;
        let (l, r, c) = coords(cell);
        log::info!(
            "side {side} takes level {} row {} col {}",
            l + 1,
            r + 1,
            c + 1
        );
        if let Some((w, line)) = winner(&self.board, &self.lines) {
            self.win_line = Some(line);
            self.end(w);
        } else if self.board.iter().all(|&v| v != 0) {
            self.end(0);
        } else {
            self.turn = if side == 1 { Turn::Computer } else { Turn::You };
            self.think = 0.0;
            if self.turn == Turn::You {
                let t = threats(&self.board, 2, &self.lines);
                self.note = if t.len() >= 2 {
                    "It has two lines of three. Block what you can!".into()
                } else if !t.is_empty() {
                    "Careful: it has three in a line.".into()
                } else {
                    "Your move.".into()
                };
            }
        }
    }

    fn end(&mut self, who: u8) {
        self.turn = Turn::Over(who);
        let (k, msg, snd) = match who {
            1 => ("won", "Four in a row! You win.", Sound::Win),
            2 => ("lost", "The computer has four in a row.", Sound::Lose),
            _ => ("drawn", "The cube is full: a draw.", Sound::Bad),
        };
        self.note = format!("{msg} Enter for another game.");
        self.ctx.audio.play(snd);
        log::info!("game over: {k} in {} moves", self.moves);
        if !self.ctx.demo {
            let key = format!("{}_{k}", self.level.name());
            let n = self.score.get::<u32>(&key).unwrap_or(0) + 1;
            self.score.set(&key, n);
            self.score.set("level", self.level.name());
            self.score.save(Self::ID, "score");
        }
    }

    fn try_place(&mut self, cell: usize) {
        if self.turn != Turn::You {
            return;
        }
        if self.board[cell] != 0 {
            self.ctx.audio.play(Sound::Nope);
            self.note = "That cell is taken.".into();
            return;
        }
        self.ctx.audio.play(Sound::Click);
        self.place(cell, 1);
    }

    fn move_cursor(&mut self, dl: i32, dr: i32, dc: i32) {
        let (l, r, c) = coords(self.cursor);
        let w = |v: usize, d: i32| ((v as i32 + d).rem_euclid(N as i32)) as usize;
        self.cursor = idx(w(l, dl), w(r, dr), w(c, dc));
        self.ctx.audio.play(Sound::Tick);
    }

    fn cycle_level(&mut self) {
        self.level = match self.level {
            Level::Easy => Level::Fair,
            Level::Fair => Level::Hard,
            Level::Hard => Level::Easy,
        };
        if !self.ctx.demo {
            self.score.set("level", self.level.name());
            self.score.save(Self::ID, "score");
        }
        self.note = format!(
            "The computer plays {} from the next game.",
            self.level.name()
        );
    }

    fn draw_level(&mut self, buf: &mut Buffer, x: u16, y: u16, level: usize) {
        let r = panel(
            buf,
            Rect::new(x, y, 23, 9),
            &format!("Level {}", level + 1),
            if coords(self.cursor).0 == level {
                AMBER
            } else {
                DIM
            },
        );
        // a 4x4 grid: cells three wide, lines between
        for row in 0..N {
            let cy = r.y + row as u16 * 2;
            for col in 0..N {
                let cx = r.x + col as u16 * 5;
                let i = idx(level, row, col);
                let in_win = self.win_line.is_some_and(|w| w.contains(&i));
                let is_cursor = i == self.cursor && matches!(self.turn, Turn::You);
                let (s, fg) = match self.board[i] {
                    1 => ("X", AMBER),
                    2 => ("O", CYAN),
                    _ => ("·", LINE),
                };
                let bg = if in_win {
                    GREEN
                } else if is_cursor {
                    Color::Rgb(70, 56, 20)
                } else if Some(i) == self.last {
                    Color::Rgb(30, 44, 52)
                } else {
                    BG
                };
                let fg = if in_win { BG } else { fg };
                let cell = Rect::new(cx, cy, 3, 1);
                fill(buf, cell, on(fg, bg));
                put(buf, cx + 1, cy, s, bold(on(fg, bg)));
                self.cells.push((cell, i));
                if col + 1 < N {
                    put(buf, cx + 3, cy, " │", st(LINE));
                }
            }
            if row + 1 < N {
                put(buf, r.x, cy + 1, "────┼────┼────┼───", st(LINE));
            }
        }
    }
}

use ratatui::style::Color;

impl Game for Qubic {
    const ID: &'static str = "3d-tic-tac-toe";
    const TITLE: &'static str = "3-D Tic-Tac-Toe";
    const HELP: &'static str = "Tic-tac-toe in a cube of four levels, each four by four. Take turns
placing your X; the first to get four in a line wins. Lines run along a
level, straight down through the levels, or on any diagonal, 76 in all.
Arrows move on a level, PgUp and PgDn (or [ and ]) change level, Enter or
Space places, or click a cell. L sets the computer's strength for the next
game, N starts again, Tab shows the rules.";

    fn new(ctx: Ctx) -> Self {
        let score = if ctx.demo {
            Kv::default()
        } else {
            Kv::load(Self::ID, "score")
        };
        let level = match score.0.get("level").map(|s| s.as_str()) {
            Some("easy") => Level::Easy,
            Some("hard") => Level::Hard,
            _ => Level::Fair,
        };
        let mut g = Qubic {
            ctx,
            lines: lines(),
            board: [0; CELLS],
            turn: Turn::You,
            first: 2,
            level,
            cursor: idx(1, 1, 1),
            last: None,
            win_line: None,
            think: 0.0,
            note: String::new(),
            score,
            show_help: false,
            quit: false,
            cells: Vec::new(),
            buttons: Vec::new(),
            demo_clock: 0.0,
            moves: 0,
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
        match k {
            Key::Esc | Key::Char('q') => self.quit = true,
            Key::Tab | Key::Char('?') => self.show_help = !self.show_help,
            Key::Char('m') => {
                self.ctx.audio.toggle();
                crate::shell::remember_mute(Self::ID, self.ctx.audio.muted);
            }
            Key::Char('n') => self.new_game(),
            Key::Char('l') => self.cycle_level(),
            Key::Up => self.move_cursor(0, -1, 0),
            Key::Down => self.move_cursor(0, 1, 0),
            Key::Left => self.move_cursor(0, 0, -1),
            Key::Right => self.move_cursor(0, 0, 1),
            Key::PageUp | Key::Char('[') => self.move_cursor(-1, 0, 0),
            Key::PageDown | Key::Char(']') => self.move_cursor(1, 0, 0),
            Key::Char(c @ '1'..='4') => {
                let (_, r, col) = coords(self.cursor);
                self.cursor = idx(c as usize - '1' as usize, r, col);
            }
            Key::Enter | Key::Char(' ') => match self.turn {
                Turn::You => self.try_place(self.cursor),
                Turn::Over(_) => self.new_game(),
                Turn::Computer => {}
            },
            _ => {}
        }
    }

    fn mouse(&mut self, m: Mouse) {
        match m.kind {
            MouseKind::Click => {
                if let Some((_, i)) = self.cells.iter().find(|(r, _)| hit(r, m.x, m.y)).copied() {
                    if matches!(self.turn, Turn::Over(_)) {
                        self.new_game();
                    } else {
                        self.cursor = i;
                        self.try_place(i);
                    }
                } else if let Some((_, k)) =
                    self.buttons.iter().find(|(r, _)| hit(r, m.x, m.y)).cloned()
                {
                    self.key(k);
                }
            }
            MouseKind::Move => {
                if let Some((_, i)) = self.cells.iter().find(|(r, _)| hit(r, m.x, m.y)).copied() {
                    self.cursor = i;
                }
            }
            _ => {}
        }
    }

    fn tick(&mut self, dt: f32) {
        if self.ctx.demo {
            self.demo_clock += dt;
            match self.turn {
                Turn::Over(_) => {
                    if self.demo_clock > 4.0 {
                        self.demo_clock = 0.0;
                        self.new_game();
                    }
                }
                _ => {
                    if self.demo_clock > 0.7 {
                        self.demo_clock = 0.0;
                        let side = if self.turn == Turn::You { 1 } else { 2 };
                        let lv = if side == 1 { Level::Fair } else { Level::Hard };
                        let c = choose(&self.board, side, lv, &mut self.ctx.rng, &self.lines);
                        self.cursor = c;
                        self.ctx.audio.play(Sound::Tick);
                        self.place(c, side);
                    }
                }
            }
            return;
        }
        if self.turn == Turn::Computer {
            self.think += dt;
            if self.think > 0.5 {
                let c = choose(&self.board, 2, self.level, &mut self.ctx.rng, &self.lines);
                self.ctx.audio.play(Sound::Tick);
                self.place(c, 2);
            }
        }
    }

    fn quit(&self) -> bool {
        self.quit
    }

    fn snapshot(&self) -> String {
        format!(
            "turn {:?} moves {} cursor {:?} level {:?}",
            self.turn,
            self.moves,
            coords(self.cursor),
            self.level
        )
    }

    fn draw(&mut self, area: Rect, buf: &mut Buffer) {
        self.cells.clear();
        self.buttons.clear();
        let status = format!("computer: {}", self.level.name());
        let inner = frame(
            buf,
            area,
            Self::TITLE,
            &status,
            &[
                ("←↑↓→", "move"),
                ("PgUp/Dn", "level"),
                ("Enter", "place"),
                ("L", "strength"),
                ("N", "new"),
                ("Tab", "rules"),
                ("M", "sound"),
                ("Esc", "quit"),
            ],
        );
        if too_small(buf, inner, 78, 21) {
            return;
        }
        // the four levels, two by two, the top of the cube first
        let bx = inner.x + 1;
        let by = inner.y + 1;
        for level in 0..N {
            let x = bx + (level % 2) as u16 * 24;
            let y = by + (level / 2) as u16 * 10;
            self.draw_level(buf, x, y, level);
        }
        if inner.height >= 21 {
            put_max(
                buf,
                bx,
                by + 19,
                "Level 1 is the top of the cube, 4 the bottom.",
                st(DIM),
                47,
            );
        }

        let rx = bx + 49;
        let right = Rect::new(
            rx,
            inner.y + 1,
            inner.right().saturating_sub(rx + 1),
            inner.height - 2,
        );
        let p = panel(
            buf,
            Rect {
                height: 8.min(right.height),
                ..right
            },
            "The game",
            AMBER,
        );
        let who = match self.turn {
            Turn::You => ("Your move (X)", AMBER),
            Turn::Computer => ("The computer is thinking (O)", CYAN),
            Turn::Over(1) => ("You won", GREEN),
            Turn::Over(2) => ("The computer won", RED),
            Turn::Over(_) => ("A draw", INK),
        };
        put_max(buf, p.x, p.y, who.0, bold(st(who.1)), p.width);
        let (l, r, c) = coords(self.cursor);
        put_max(
            buf,
            p.x,
            p.y + 1,
            &format!("level {}, row {}, column {}", l + 1, r + 1, c + 1),
            st(DIM),
            p.width,
        );
        let mine = threats(&self.board, 1, &self.lines).len();
        let its = threats(&self.board, 2, &self.lines).len();
        put_max(
            buf,
            p.x,
            p.y + 2,
            &format!("threes: you {mine}, computer {its}"),
            st(INK),
            p.width,
        );
        para(
            buf,
            Rect::new(p.x, p.y + 4, p.width, p.height.saturating_sub(4)),
            &self.note,
            st(match self.turn {
                Turn::Over(1) => GREEN,
                Turn::Over(2) => RED,
                _ => CYAN,
            }),
        );

        let lower = Rect {
            y: right.y + 8,
            height: right.height.saturating_sub(8),
            ..right
        };
        if self.show_help {
            let p = panel(buf, lower, "How it works", CYAN);
            para(buf, p, "Four levels make a cube. A line of four can run along a row or column of one level, straight down through all four levels, or on a diagonal: across a level, down a side, or corner to corner through the middle. 76 lines in all. Moving first is a big help; you take turns starting.", st(INK));
        } else {
            let p = panel(buf, lower, "Your record", CYAN);
            let mut y = p.y;
            put(buf, p.x, y, "        won lost drawn", st(DIM));
            y += 1;
            for lv in [Level::Easy, Level::Fair, Level::Hard] {
                if y >= p.bottom() {
                    break;
                }
                let g = |k: &str| {
                    self.score
                        .get::<u32>(&format!("{}_{k}", lv.name()))
                        .unwrap_or(0)
                };
                let line = format!(
                    "{:<6}{:>5}{:>5}{:>7}",
                    lv.name(),
                    g("won"),
                    g("lost"),
                    g("drawn")
                );
                let row = Rect::new(p.x, y, p.width, 1);
                put_max(
                    buf,
                    p.x,
                    y,
                    &line,
                    st(if lv == self.level { AMBER } else { INK }),
                    p.width,
                );
                self.buttons.push((row, Key::Char('l')));
                y += 1;
            }
            if y + 1 < p.bottom() {
                put_max(
                    buf,
                    p.x,
                    y + 1,
                    "L or a click: its strength",
                    st(DIM),
                    p.width,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn there_are_76_lines() {
        let ls = lines();
        assert_eq!(ls.len(), 76);
        // every corner lies on seven lines, every inner cell too
        let on = |c: usize| ls.iter().filter(|l| l.contains(&c)).count();
        assert_eq!(on(idx(0, 0, 0)), 7);
        assert_eq!(on(idx(1, 1, 1)), 7);
        assert_eq!(on(idx(0, 0, 1)), 4);
    }

    #[test]
    fn the_computer_wins_and_blocks() {
        let ls = lines();
        let mut rng = Rng::new(3);
        let mut b = [0; CELLS];
        // the player has three down the levels at row 0 column 0
        for l in 0..3 {
            b[idx(l, 0, 0)] = 1;
        }
        assert_eq!(choose(&b, 2, Level::Fair, &mut rng, &ls), idx(3, 0, 0));
        // with three of its own in a line as well, it takes the win instead
        for l in 0..3 {
            b[idx(l, 1, 1)] = 2;
        }
        assert_eq!(choose(&b, 2, Level::Fair, &mut rng, &ls), idx(3, 1, 1));
    }

    #[test]
    fn a_game_between_computers_ends() {
        let ls = lines();
        let mut rng = Rng::new(11);
        let mut b = [0; CELLS];
        let mut side = 1;
        for _ in 0..CELLS {
            let c = choose(&b, side, Level::Fair, &mut rng, &ls);
            assert_eq!(b[c], 0);
            b[c] = side;
            if winner(&b, &ls).is_some() {
                return;
            }
            side = 3 - side;
        }
        assert!(b.iter().all(|&v| v != 0));
    }
}
