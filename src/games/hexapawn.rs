//! Hexapawn: three pawns a side on a 3x3 board, against a computer that
//! starts out knowing only the rules and learns from every game it loses.
//! Rules in asciiverse/specs/hexapawn.md.

use crate::audio::Sound;
use crate::rng::Rng;
use crate::shell::{Ctx, Game, Key, Mouse, MouseKind};
use crate::store::Kv;
use crate::ui::*;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use std::collections::BTreeMap;

/// 0 empty, 1 yours (moving up), 2 the computer's (moving down); index
/// row * 3 + column, row 0 at the top.
pub type Board = [u8; 9];

pub const START: Board = [2, 2, 2, 0, 0, 0, 1, 1, 1];

pub type Move = (usize, usize);

pub fn moves(b: &Board, side: u8) -> Vec<Move> {
    let dir: i32 = if side == 1 { -1 } else { 1 };
    let other = 3 - side;
    let mut out = Vec::new();
    for from in 0..9 {
        if b[from] != side {
            continue;
        }
        let (r, c) = ((from / 3) as i32, (from % 3) as i32);
        let nr = r + dir;
        if !(0..3).contains(&nr) {
            continue;
        }
        let ahead = (nr * 3 + c) as usize;
        if b[ahead] == 0 {
            out.push((from, ahead));
        }
        for dc in [-1, 1] {
            let nc = c + dc;
            if (0..3).contains(&nc) {
                let to = (nr * 3 + nc) as usize;
                if b[to] == other {
                    out.push((from, to));
                }
            }
        }
    }
    out
}

pub fn play(b: &Board, m: Move) -> Board {
    let mut n = *b;
    n[m.1] = n[m.0];
    n[m.0] = 0;
    n
}

/// Who has won, if anyone, with `to_move` about to play.
pub fn winner(b: &Board, to_move: u8) -> Option<u8> {
    if b[0..3].contains(&1) {
        return Some(1);
    }
    if b[6..9].contains(&2) {
        return Some(2);
    }
    if !b.contains(&1) {
        return Some(2);
    }
    if !b.contains(&2) {
        return Some(1);
    }
    if moves(b, to_move).is_empty() {
        return Some(3 - to_move);
    }
    None
}

pub fn key_of(b: &Board) -> String {
    b.iter().map(|v| char::from(b'0' + v)).collect()
}

/// The computer's memory: for each position it has met, the moves it still
/// trusts. Losing a game strikes out the last move it made.
#[derive(Clone, Debug, Default)]
pub struct Memory {
    pub boxes: BTreeMap<String, Vec<Move>>,
    pub struck: u32,
}

impl Memory {
    pub fn options(&mut self, b: &Board) -> &Vec<Move> {
        self.boxes.entry(key_of(b)).or_insert_with(|| moves(b, 2))
    }

    /// Strike out `m` from position `b`; true when that empties the box.
    pub fn strike(&mut self, b: &Board, m: Move) -> bool {
        let v = self.boxes.entry(key_of(b)).or_insert_with(|| moves(b, 2));
        let before = v.len();
        v.retain(|x| *x != m);
        if v.len() < before {
            self.struck += 1;
        }
        v.is_empty()
    }

    pub fn save(&self) -> String {
        let mut out = format!("struck={}\n", self.struck);
        for (k, v) in &self.boxes {
            let ms: Vec<String> = v.iter().map(|(a, b)| format!("{a}{b}")).collect();
            out += &format!("{k}:{}\n", ms.join(","));
        }
        out
    }

    pub fn load(text: &str) -> Memory {
        let mut m = Memory::default();
        for line in text.lines() {
            if let Some(n) = line.strip_prefix("struck=") {
                m.struck = n.parse().unwrap_or(0);
            } else if let Some((k, v)) = line.split_once(':') {
                if k.len() != 9 {
                    continue;
                }
                let ms = v
                    .split(',')
                    .filter_map(|s| {
                        let b = s.as_bytes();
                        (b.len() == 2).then(|| ((b[0] - b'0') as usize, (b[1] - b'0') as usize))
                    })
                    .filter(|(a, b)| *a < 9 && *b < 9)
                    .collect();
                m.boxes.insert(k.to_string(), ms);
            }
        }
        m
    }

    /// Pick a move it still trusts, or None to resign.
    pub fn choose(&mut self, b: &Board, rng: &mut Rng) -> Option<Move> {
        let o = self.options(b).clone();
        if o.is_empty() {
            return None;
        }
        Some(o[rng.range(0, o.len() as i64 - 1) as usize])
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Turn {
    You,
    Thinking,
    Over(u8),
}

pub struct Hexapawn {
    ctx: Ctx,
    board: Board,
    turn: Turn,
    memory: Memory,
    /// the computer's moves this game, with the positions they came from
    trail: Vec<(Board, Move)>,
    cursor: usize,
    picked: Option<usize>,
    think: f32,
    results: Vec<bool>,
    score: Kv,
    note: String,
    resigned: bool,
    quit: bool,
    cells: Vec<(Rect, usize)>,
    demo_clock: f32,
}

impl Hexapawn {
    fn new_game(&mut self) {
        self.board = START;
        self.turn = Turn::You;
        self.trail.clear();
        self.picked = None;
        self.cursor = 7;
        self.resigned = false;
        self.note = "Your move: you play the bottom row, upward.".into();
    }

    fn save(&self) {
        if self.ctx.demo {
            return;
        }
        crate::store::write(Self::ID, "memory", &self.memory.save());
        self.score.save(Self::ID, "score");
    }

    fn end(&mut self, who: u8) {
        self.turn = Turn::Over(who);
        let computer_won = who == 2;
        self.results.push(computer_won);
        if self.results.len() > 40 {
            self.results.remove(0);
        }
        if !computer_won {
            // learn: strike out the last move; if that leaves the position
            // with nothing, strike the move that led there too
            let mut i = self.trail.len();
            while i > 0 {
                i -= 1;
                let (b, m) = self.trail[i];
                if !self.memory.strike(&b, m) {
                    break;
                }
            }
            self.ctx.audio.play(Sound::Win);
            self.note = if self.resigned {
                "It resigns: every move it has left has lost before. You win!".into()
            } else {
                "You win! It will not make that last move again.".into()
            };
        } else {
            self.ctx.audio.play(Sound::Lose);
            self.note = "The computer wins.".into();
        }
        let k = if computer_won { "computer" } else { "you" };
        let n = self.score.get::<u32>(k).unwrap_or(0) + 1;
        self.score.set(k, n);
        self.save();
        log::info!(
            "game over, winner {who}, boxes {} struck {}",
            self.memory.boxes.len(),
            self.memory.struck
        );
    }

    fn after_move(&mut self, next: u8) {
        if let Some(w) = winner(&self.board, next) {
            self.end(w);
        }
    }

    fn try_move(&mut self, from: usize, to: usize) -> bool {
        if self.turn != Turn::You || !moves(&self.board, 1).contains(&(from, to)) {
            return false;
        }
        self.board = play(&self.board, (from, to));
        self.picked = None;
        self.ctx.audio.play(Sound::Click);
        self.turn = Turn::Thinking;
        self.think = 0.0;
        self.note = String::new();
        self.after_move(2);
        true
    }

    fn select(&mut self, sq: usize) {
        if self.turn != Turn::You {
            return;
        }
        if self.board[sq] == 1 {
            self.picked = Some(sq);
            self.ctx.audio.play(Sound::Tick);
        } else if let Some(from) = self.picked {
            if !self.try_move(from, sq) {
                self.ctx.audio.play(Sound::Nope);
                self.note =
                    "Pawns step straight ahead onto an empty square, or take on the diagonal."
                        .into();
            }
        }
    }

    fn computer(&mut self) {
        let b = self.board;
        match self.memory.choose(&b, &mut self.ctx.rng) {
            Some(m) => {
                self.trail.push((b, m));
                self.board = play(&b, m);
                self.ctx.audio.play(Sound::Tick);
                self.turn = Turn::You;
                self.note = "Your move.".into();
                self.after_move(1);
            }
            None => {
                self.resigned = true;
                self.end(1);
            }
        }
    }
}

impl Game for Hexapawn {
    const ID: &'static str = "hexapawn";
    const TITLE: &'static str = "Hexapawn";
    const HELP: &'static str = "Three pawns each on a 3x3 board. Pawns step straight ahead onto an
empty square or take an enemy pawn diagonally ahead. Win by reaching the
far row, taking every enemy pawn, or leaving the other side with no move.
You move first. The computer knows only the rules; each time it loses, it
strikes out the move that lost, and in time it cannot be beaten.
Arrows and Enter (or the mouse) pick a pawn and its square. R forgets
everything it has learned.";

    fn new(ctx: Ctx) -> Self {
        let memory = if ctx.demo {
            Memory::default()
        } else {
            crate::store::read("hexapawn", "memory")
                .map(|t| Memory::load(&t))
                .unwrap_or_default()
        };
        let mut g = Hexapawn {
            ctx,
            board: START,
            turn: Turn::You,
            memory,
            trail: Vec::new(),
            cursor: 7,
            picked: None,
            think: 0.0,
            results: Vec::new(),
            score: Kv::load("hexapawn", "score"),
            note: String::new(),
            resigned: false,
            quit: false,
            cells: Vec::new(),
            demo_clock: 0.0,
        };
        if g.ctx.demo {
            g.score = Kv::default();
        }
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
            Key::Char('m') => {
                self.ctx.audio.toggle();
                crate::shell::remember_mute(Self::ID, self.ctx.audio.muted);
            }
            Key::Char('r') => {
                self.memory = Memory::default();
                self.results.clear();
                self.score = Kv::default();
                self.save();
                self.new_game();
                self.note = "Memory wiped: it knows only the rules again.".into();
                self.ctx.audio.play(Sound::Bad);
            }
            Key::Enter | Key::Char(' ') if matches!(self.turn, Turn::Over(_)) => self.new_game(),
            Key::Up if self.cursor >= 3 => self.cursor -= 3,
            Key::Down if self.cursor < 6 => self.cursor += 3,
            Key::Left if !self.cursor.is_multiple_of(3) => self.cursor -= 1,
            Key::Right if self.cursor % 3 < 2 => self.cursor += 1,
            Key::Enter | Key::Char(' ') => self.select(self.cursor),
            Key::Backspace => self.picked = None,
            Key::Char(c @ '1'..='9') => {
                // the number pad's layout: 7 8 9 on top
                let n = (c as u8 - b'1') as usize;
                let sq = (2 - n / 3) * 3 + n % 3;
                self.cursor = sq;
                self.select(sq);
            }
            _ => {}
        }
    }

    fn mouse(&mut self, m: Mouse) {
        if let Some(&(_, sq)) = self.cells.iter().find(|(r, _)| hit(r, m.x, m.y)) {
            match m.kind {
                MouseKind::Click => {
                    self.cursor = sq;
                    if matches!(self.turn, Turn::Over(_)) {
                        self.new_game();
                    } else {
                        self.select(sq);
                    }
                }
                MouseKind::Move => self.cursor = sq,
                _ => {}
            }
        }
    }

    fn tick(&mut self, dt: f32) {
        if self.turn == Turn::Thinking {
            self.think += dt;
            if self.think > 0.6 {
                self.computer();
            }
        }
        if !self.ctx.demo {
            return;
        }
        self.demo_clock += dt;
        if self.demo_clock < 0.9 {
            return;
        }
        self.demo_clock = 0.0;
        match self.turn {
            Turn::Over(_) => self.new_game(),
            Turn::You => {
                let ms = moves(&self.board, 1);
                if let Some(&m) = ms.get(self.ctx.rng.range(0, ms.len() as i64 - 1) as usize) {
                    // a demo player who likes to win: take a winning move when there is one
                    let win = ms
                        .iter()
                        .find(|&&x| winner(&play(&self.board, x), 2) == Some(1))
                        .copied();
                    let m = win.unwrap_or(m);
                    self.cursor = m.1;
                    self.try_move(m.0, m.1);
                }
            }
            Turn::Thinking => {}
        }
    }

    fn quit(&self) -> bool {
        self.quit
    }

    fn snapshot(&self) -> String {
        format!(
            "board {} turn {:?} cursor {} picked {:?} boxes {} struck {}",
            key_of(&self.board),
            self.turn,
            self.cursor,
            self.picked,
            self.memory.boxes.len(),
            self.memory.struck
        )
    }

    fn draw(&mut self, area: Rect, buf: &mut Buffer) {
        self.cells.clear();
        let you = self.score.get::<u32>("you").unwrap_or(0);
        let it = self.score.get::<u32>("computer").unwrap_or(0);
        let status = format!("you {you}  computer {it}");
        let keys: &[(&str, &str)] = if matches!(self.turn, Turn::Over(_)) {
            &[
                ("Enter", "play again"),
                ("R", "wipe its memory"),
                ("Esc", "quit"),
            ]
        } else {
            &[
                ("arrows", "move"),
                ("Enter", "pick/move"),
                ("1-9", "square"),
                ("R", "wipe its memory"),
                ("M", "sound"),
                ("Esc", "quit"),
            ]
        };
        let inner = frame(buf, area, "Hexapawn", &status, keys);
        if too_small(buf, inner, 70, 22) {
            return;
        }
        let board_w = 40.min(inner.width / 2);
        let left = Rect {
            width: board_w,
            ..inner
        };
        let right = Rect {
            x: inner.x + board_w,
            width: inner.width - board_w,
            ..inner
        };

        let p = panel(buf, left, "Board", AMBER);
        let cw: u16 = 10;
        let ch: u16 = 5;
        let bx = p.x + p.width.saturating_sub(cw * 3) / 2;
        let by = p.y + 1;
        let legal: Vec<usize> = match self.picked {
            Some(f) => moves(&self.board, 1)
                .iter()
                .filter(|m| m.0 == f)
                .map(|m| m.1)
                .collect(),
            None => Vec::new(),
        };
        let last = self.trail.last().map(|(_, m)| *m);
        for sq in 0..9 {
            let (r, c) = ((sq / 3) as u16, (sq % 3) as u16);
            let cell = Rect::new(bx + c * cw, by + r * ch, cw, ch);
            let dark = (r + c) % 2 == 1;
            let mut bg = if dark {
                ratatui::style::Color::Rgb(54, 46, 36)
            } else {
                ratatui::style::Color::Rgb(92, 80, 62)
            };
            if legal.contains(&sq) {
                bg = mix(bg, GREEN, 0.35);
            }
            if last.is_some_and(|m| m.0 == sq || m.1 == sq) && self.turn != Turn::Thinking {
                bg = mix(bg, BLUE, 0.25);
            }
            fill(buf, cell, on(INK, bg));
            let piece = self.board[sq];
            if piece != 0 {
                let fg = if piece == 1 {
                    ratatui::style::Color::Rgb(250, 244, 230)
                } else {
                    ratatui::style::Color::Rgb(20, 18, 16)
                };
                let px = cell.x + cw / 2 - 2;
                put(buf, px + 1, cell.y + 1, "▄▄", on(fg, bg));
                put(buf, px + 1, cell.y + 2, "██", on(fg, bg));
                put(buf, px, cell.y + 3, "▀██▀", on(fg, bg));
            }
            if self.picked == Some(sq) {
                put(buf, cell.x, cell.y, "▛", bold(on(AMBER, bg)));
                put(
                    buf,
                    cell.right() - 1,
                    cell.bottom() - 1,
                    "▟",
                    bold(on(AMBER, bg)),
                );
            }
            if self.cursor == sq && self.turn == Turn::You {
                put(buf, cell.x, cell.bottom() - 1, "▙", bold(on(AMBER, bg)));
                put(buf, cell.right() - 1, cell.y, "▜", bold(on(AMBER, bg)));
            }
            self.cells.push((cell, sq));
        }
        let ny = by + ch * 3 + 1;
        let note = match self.turn {
            Turn::Thinking => "The computer is thinking...".to_string(),
            _ => self.note.clone(),
        };
        para(
            buf,
            Rect::new(p.x, ny, p.width, p.bottom().saturating_sub(ny)),
            &note,
            st(match self.turn {
                Turn::Over(1) => GREEN,
                Turn::Over(_) => RED,
                _ => CYAN,
            }),
        );

        // what the computer has learned
        let m = panel(buf, right, "The computer's memory", CYAN);
        let lines = [
            format!("positions met       {}", self.memory.boxes.len()),
            format!("moves struck out    {}", self.memory.struck),
            format!("games: you {you}, it {it}"),
        ];
        for (i, l) in lines.iter().enumerate() {
            put(buf, m.x, m.y + i as u16, l, st(INK));
        }
        // the run of recent games: green you, red it
        let y = m.y + 4;
        put(buf, m.x, y, "recent games", st(DIM));
        let mut x = m.x + 13;
        for &w in self
            .results
            .iter()
            .rev()
            .take(m.width.saturating_sub(14) as usize)
            .collect::<Vec<_>>()
            .iter()
            .rev()
        {
            put(buf, x, y, "▮", st(if *w { RED } else { GREEN }));
            x += 1;
        }
        // the moves it still trusts from this position
        let y = y + 2;
        let b = self.board;
        if self.turn != Turn::You || self.trail.is_empty() {
            put(buf, m.x, y, "Its choices here:", st(DIM));
        } else {
            put(buf, m.x, y, "Its choices last move:", st(DIM));
        }
        let at = if self.turn == Turn::You {
            self.trail.last().map(|(b, _)| *b)
        } else {
            Some(b)
        };
        if let Some(pos) = at {
            let all = moves(&pos, 2);
            let kept = self
                .memory
                .boxes
                .get(&key_of(&pos))
                .cloned()
                .unwrap_or_else(|| all.clone());
            let name = |s: usize| format!("{}{}", ['a', 'b', 'c'][s % 3], 3 - s / 3);
            for (i, mv) in all.iter().enumerate() {
                let yy = y + 1 + i as u16;
                if yy >= m.bottom() {
                    break;
                }
                let ok = kept.contains(mv);
                let s = format!(
                    "{} {} → {}",
                    if ok { "✓" } else { "✗" },
                    name(mv.0),
                    name(mv.1)
                );
                put(buf, m.x + 2, yy, &s, st(if ok { GREEN } else { LINE }));
            }
        }
        let fy = m.bottom().saturating_sub(2);
        para(
            buf,
            Rect::new(m.x, fy, m.width, 2),
            "Each loss strikes out a move for good. Squares: a-c left to right, 1-3 bottom to top.",
            st(DIM),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opening_moves() {
        assert_eq!(moves(&START, 1).len(), 3);
        assert_eq!(moves(&START, 2).len(), 3);
    }

    #[test]
    fn captures_and_wins() {
        let b = [2, 2, 2, 1, 0, 0, 0, 1, 1];
        assert!(moves(&b, 2).contains(&(1, 3)));
        assert_eq!(winner(&[1, 0, 0, 0, 0, 0, 0, 0, 2], 2), Some(1));
        // blocked pawns: nobody can move, so the side to move loses
        assert_eq!(winner(&[0, 2, 0, 0, 1, 0, 0, 0, 0], 2), Some(1));
    }

    #[test]
    fn it_learns_until_it_cannot_lose() {
        let mut rng = Rng::new(11);
        let mut mem = Memory::default();
        let mut late_losses = 0;
        for game in 0..400 {
            let mut b = START;
            let mut trail: Vec<(Board, Move)> = Vec::new();
            let mut side = 1u8;
            let result = loop {
                if let Some(w) = winner(&b, side) {
                    break w;
                }
                if side == 1 {
                    let ms = moves(&b, 1);
                    let m = ms[rng.range(0, ms.len() as i64 - 1) as usize];
                    b = play(&b, m);
                } else {
                    match mem.choose(&b, &mut rng) {
                        Some(m) => {
                            trail.push((b, m));
                            b = play(&b, m);
                        }
                        None => break 1,
                    }
                }
                side = 3 - side;
            };
            if result == 1 {
                let mut i = trail.len();
                while i > 0 {
                    i -= 1;
                    let (pb, m) = trail[i];
                    if !mem.strike(&pb, m) {
                        break;
                    }
                }
                if game >= 300 {
                    late_losses += 1;
                }
            }
        }
        assert_eq!(late_losses, 0);
        let back = Memory::load(&mem.save());
        assert_eq!(back.boxes, mem.boxes);
    }
}
