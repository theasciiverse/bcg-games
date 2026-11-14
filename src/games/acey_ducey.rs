//! Acey Ducey: two cards face up, and a bet that the next one falls
//! between them. Rules in asciiverse/specs/acey-ducey.md.

use crate::audio::Sound;
use crate::rng::Rng;
use crate::shell::{Ctx, Game, Key, Mouse, MouseKind};
use crate::store::Kv;
use crate::ui::*;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

pub const START_PURSE: i64 = 100;

/// A card: rank 2 to 14 (11 jack, 12 queen, 13 king, 14 ace), suit 0 to 3.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Card {
    pub rank: u8,
    pub suit: u8,
}

impl Card {
    pub fn rank_name(&self) -> &'static str {
        [
            "2", "3", "4", "5", "6", "7", "8", "9", "10", "J", "Q", "K", "A",
        ][(self.rank - 2) as usize]
    }

    pub fn long_name(&self) -> String {
        let r = match self.rank {
            11 => "jack".to_string(),
            12 => "queen".to_string(),
            13 => "king".to_string(),
            14 => "ace".to_string(),
            n => n.to_string(),
        };
        let s = ["spades", "hearts", "diamonds", "clubs"][self.suit as usize];
        format!("{r} of {s}")
    }

    pub fn suit_sym(&self) -> &'static str {
        ["♠", "♥", "♦", "♣"][self.suit as usize]
    }

    pub fn red(&self) -> bool {
        self.suit == 1 || self.suit == 2
    }
}

/// A shuffled 52-card shoe; shuffled afresh when it runs low.
pub struct Shoe {
    pub cards: Vec<Card>,
}

impl Shoe {
    pub fn new(rng: &mut Rng) -> Shoe {
        let mut cards = Vec::with_capacity(52);
        for suit in 0..4 {
            for rank in 2..=14 {
                cards.push(Card { rank, suit });
            }
        }
        for i in (1..cards.len()).rev() {
            let j = rng.range(0, i as i64) as usize;
            cards.swap(i, j);
        }
        Shoe { cards }
    }

    pub fn draw(&mut self, rng: &mut Rng) -> Card {
        if self.cards.is_empty() {
            *self = Shoe::new(rng);
        }
        self.cards.pop().unwrap()
    }
}

/// True when `c` falls strictly between `a` and `b`.
pub fn between(a: Card, b: Card, c: Card) -> bool {
    let (lo, hi) = (a.rank.min(b.rank), a.rank.max(b.rank));
    c.rank > lo && c.rank < hi
}

/// The chance that the next card from these left in the shoe wins.
pub fn chance(a: Card, b: Card, left: &[Card]) -> f32 {
    if left.is_empty() {
        return 0.0;
    }
    let n = left.iter().filter(|c| between(a, b, **c)).count();
    n as f32 / left.len() as f32
}

/// Deals a pair with at least one rank between them, low card first.
/// Pairs too close to bet on go back under the shoe.
pub fn deal_pair(shoe: &mut Shoe, rng: &mut Rng) -> (Card, Card) {
    let mut tries = 0;
    loop {
        // a shoe whose last cards are all too close together is shuffled anew
        if shoe.cards.len() < 6 || tries > shoe.cards.len() {
            *shoe = Shoe::new(rng);
            tries = 0;
        }
        tries += 2;
        let a = shoe.draw(rng);
        let b = shoe.draw(rng);
        if (a.rank as i32 - b.rank as i32).abs() >= 2 {
            return if a.rank <= b.rank { (a, b) } else { (b, a) };
        }
        shoe.cards.insert(0, a);
        shoe.cards.insert(0, b);
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Phase {
    /// two cards up, waiting for a bet
    Betting,
    /// the third card turning over
    Turning,
    /// the hand's result on show
    Shown,
    Broke,
    WalkedAway,
}

pub struct AceyDucey {
    ctx: Ctx,
    shoe: Shoe,
    pair: (Card, Card),
    third: Option<Card>,
    purse: i64,
    peak: i64,
    bet: NumField,
    fresh: bool,
    last_bet: i64,
    phase: Phase,
    turn_clock: f32,
    hands: u32,
    log: Vec<(String, Color)>,
    note: String,
    stats: Kv,
    show_help: bool,
    quit: bool,
    buttons: Vec<(Rect, Key)>,
    demo_clock: f32,
    demo_step: u8,
}

impl AceyDucey {
    fn new_game(&mut self) {
        self.purse = START_PURSE;
        self.peak = START_PURSE;
        self.hands = 0;
        self.log.clear();
        self.shoe = Shoe::new(&mut self.ctx.rng);
        self.note = format!("You sit down with ${START_PURSE}. Bet on the card between.");
        log::info!("new game, purse {}", self.purse);
        self.next_hand();
    }

    fn next_hand(&mut self) {
        self.pair = deal_pair(&mut self.shoe, &mut self.ctx.rng);
        self.third = None;
        self.phase = Phase::Betting;
        let suggest = self.last_bet.min(self.purse).max(0);
        self.bet = NumField::new("Your bet", suggest, 0, self.purse, 5);
        self.fresh = true;
        log::info!(
            "deal {} and {}",
            self.pair.0.long_name(),
            self.pair.1.long_name()
        );
    }

    fn place(&mut self) {
        if self.phase != Phase::Betting {
            return;
        }
        if !self.bet.valid() {
            self.note = format!("Bet from $0 to ${}.", self.purse);
            self.ctx.audio.play(Sound::Nope);
            return;
        }
        self.last_bet = self.bet.value();
        self.third = Some(self.shoe.draw(&mut self.ctx.rng));
        self.phase = Phase::Turning;
        self.turn_clock = 0.0;
        self.ctx.audio.play(Sound::Click);
        log::info!("bet {}", self.last_bet);
    }

    fn pass(&mut self) {
        if self.phase != Phase::Betting {
            return;
        }
        self.bet.set(0);
        let keep = self.last_bet;
        self.place();
        self.last_bet = keep;
    }

    fn settle(&mut self) {
        let (a, b) = self.pair;
        let c = self.third.unwrap();
        let bet = self.bet.value();
        let won = between(a, b, c);
        self.hands += 1;
        let line = format!("{}-{} {}", a.rank_name(), b.rank_name(), c.rank_name());
        if bet == 0 {
            self.note = if won {
                format!(
                    "The {} would have won. You sat this one out.",
                    c.long_name()
                )
            } else {
                format!("The {}. Good thing you passed.", c.long_name())
            };
            self.log.push((format!("{line}  passed"), DIM));
            self.ctx.audio.play(Sound::Tick);
        } else if won {
            self.purse += bet;
            self.peak = self.peak.max(self.purse);
            self.note = format!("The {}! You win ${bet}.", c.long_name());
            self.log.push((format!("{line}  +${bet}"), GREEN));
            self.ctx.audio.play(Sound::Good);
        } else {
            self.purse -= bet;
            let why = if c.rank == a.rank || c.rank == b.rank {
                "on the edge, which is not between"
            } else {
                "outside"
            };
            self.note = format!("The {}, {why}. You lose ${bet}.", c.long_name());
            self.log.push((format!("{line}  -${bet}"), RED));
            self.ctx.audio.play(Sound::Bad);
        }
        if self.log.len() > 40 {
            self.log.remove(0);
        }
        log::info!("third {} won {won} purse {}", c.long_name(), self.purse);
        if self.purse <= 0 {
            self.phase = Phase::Broke;
            self.note = format!("{} The table has the last of your money.", self.note);
            self.ctx.audio.play(Sound::Lose);
            self.record(false);
        } else {
            self.phase = Phase::Shown;
        }
    }

    fn walk_away(&mut self) {
        if !matches!(self.phase, Phase::Betting | Phase::Shown) {
            return;
        }
        self.phase = Phase::WalkedAway;
        let gain = self.purse - START_PURSE;
        self.note = if gain > 0 {
            format!("You walk away with ${}, ${gain} ahead.", self.purse)
        } else if gain == 0 {
            format!("You walk away with ${}, even.", self.purse)
        } else {
            format!("You walk away with ${}, ${} down.", self.purse, -gain)
        };
        self.ctx
            .audio
            .play(if gain > 0 { Sound::Win } else { Sound::Click });
        log::info!("walked away with {}", self.purse);
        self.record(true);
    }

    fn record(&mut self, walked: bool) {
        if self.ctx.demo {
            return;
        }
        let games = self.stats.get::<u32>("games").unwrap_or(0) + 1;
        self.stats.set("games", games);
        if walked {
            let best = self.stats.get::<i64>("best").unwrap_or(0).max(self.purse);
            self.stats.set("best", best);
        } else {
            let b = self.stats.get::<u32>("broke").unwrap_or(0) + 1;
            self.stats.set("broke", b);
        }
        let peak = self.stats.get::<i64>("peak").unwrap_or(0).max(self.peak);
        self.stats.set("peak", peak);
        self.stats.save(Self::ID, "scores");
    }

    fn go_on(&mut self) {
        match self.phase {
            Phase::Betting => self.place(),
            Phase::Turning => {}
            Phase::Shown => self.next_hand(),
            Phase::Broke | Phase::WalkedAway => self.new_game(),
        }
    }

    fn demo_tick(&mut self, dt: f32) {
        self.demo_clock += dt;
        match self.phase {
            Phase::Betting => {
                if self.demo_step == 0 && self.demo_clock > 0.9 {
                    // the demo player bets by the odds
                    let p = chance(self.pair.0, self.pair.1, &self.shoe.cards);
                    let frac = if p > 0.55 {
                        0.35
                    } else if p > 0.4 {
                        0.2
                    } else if p > 0.25 {
                        0.1
                    } else {
                        0.0
                    };
                    let b = ((self.purse as f32 * frac / 5.0).round() as i64 * 5).min(self.purse);
                    self.bet.set(b);
                    self.fresh = true;
                    self.ctx.audio.play(Sound::Tick);
                    self.demo_step = 1;
                } else if self.demo_step == 1 && self.demo_clock > 1.8 {
                    self.place();
                    self.demo_step = 0;
                    self.demo_clock = 0.0;
                }
            }
            Phase::Shown => {
                if self.demo_clock > 1.8 {
                    self.demo_clock = 0.0;
                    if self.hands >= 14 {
                        self.walk_away();
                    } else {
                        self.next_hand();
                    }
                }
            }
            Phase::Broke | Phase::WalkedAway => {
                if self.demo_clock > 3.5 {
                    self.demo_clock = 0.0;
                    self.new_game();
                }
            }
            Phase::Turning => {}
        }
    }
}

fn draw_card(buf: &mut Buffer, x: u16, y: u16, card: Option<Card>) {
    let w = 9;
    let h = 7;
    let r = Rect::new(x, y, w, h);
    match card {
        None => {
            fill(buf, r, on(BLUE, Color::Rgb(30, 40, 90)));
            for yy in y + 1..y + h - 1 {
                put(buf, x + 1, yy, "░░░░░░░", on(BLUE, Color::Rgb(30, 40, 90)));
            }
        }
        Some(c) => {
            let face = Color::Rgb(236, 230, 214);
            let ink = if c.red() {
                Color::Rgb(190, 40, 30)
            } else {
                Color::Rgb(20, 20, 24)
            };
            fill(buf, r, on(ink, face));
            put(
                buf,
                x + 1,
                y,
                &format!("{}{}", c.rank_name(), c.suit_sym()),
                bold(on(ink, face)),
            );
            let s = format!("{}{}", c.rank_name(), c.suit_sym());
            put(
                buf,
                x + w - 1 - s.chars().count() as u16,
                y + h - 1,
                &s,
                bold(on(ink, face)),
            );
            put(buf, x + 4, y + 3, c.suit_sym(), bold(on(ink, face)));
            if c.rank >= 11 {
                let big =
                    ["", "", "", "", "", "", "", "", "", "J", "Q", "K", "A"][(c.rank - 2) as usize];
                put(buf, x + 4, y + 2, big, bold(on(ink, face)));
            }
        }
    }
}

impl Game for AceyDucey {
    const ID: &'static str = "acey-ducey";
    const TITLE: &'static str = "Acey Ducey";
    const HELP: &'static str = "Two cards are dealt face up. Bet that the next card falls strictly
between them (aces are high; a card equal to either one loses), or bet
nothing and pass. You start with $100 and play until you are broke or
walk away.
Type a bet or step it with Up and Down; Enter bets, P passes, W walks
away, N starts over. Tab shows the rules, M sound, Esc quits.";

    fn new(ctx: Ctx) -> Self {
        let shoe = Shoe::new(&mut Rng::new(1));
        let mut g = AceyDucey {
            ctx,
            shoe,
            pair: (Card { rank: 2, suit: 0 }, Card { rank: 14, suit: 0 }),
            third: None,
            purse: START_PURSE,
            peak: START_PURSE,
            bet: NumField::new("Your bet", 10, 0, START_PURSE, 5),
            fresh: true,
            last_bet: 10,
            phase: Phase::Betting,
            turn_clock: 0.0,
            hands: 0,
            log: Vec::new(),
            note: String::new(),
            stats: Kv::load("acey-ducey", "scores"),
            show_help: false,
            quit: false,
            buttons: Vec::new(),
            demo_clock: 0.0,
            demo_step: 0,
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
            Key::Char('n') => {
                if matches!(self.phase, Phase::Betting | Phase::Shown) && self.hands > 0 {
                    self.walk_away();
                }
                self.new_game();
                self.ctx.audio.play(Sound::Click);
            }
            Key::Char('p') => self.pass(),
            Key::Char('w') => self.walk_away(),
            Key::Char('a') if self.phase == Phase::Betting => {
                self.bet.set(self.purse);
                self.fresh = true;
            }
            Key::Char('h') if self.phase == Phase::Betting => {
                self.bet.set(self.purse / 2);
                self.fresh = true;
            }
            Key::Enter | Key::Char(' ') => self.go_on(),
            k if self.phase == Phase::Betting => {
                if self.bet.key(&k, &mut self.fresh) {
                    self.ctx.audio.play(Sound::Tick);
                }
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
                }
            }
            MouseKind::ScrollUp if self.phase == Phase::Betting => {
                self.bet.key(&Key::Up, &mut self.fresh);
            }
            MouseKind::ScrollDown if self.phase == Phase::Betting => {
                self.bet.key(&Key::Down, &mut self.fresh);
            }
            _ => {}
        }
    }

    fn tick(&mut self, dt: f32) {
        if self.phase == Phase::Turning {
            self.turn_clock += dt;
            if self.turn_clock > 0.7 {
                self.settle();
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
            "phase {:?} purse {} pair {}-{} bet {} hands {}",
            self.phase,
            self.purse,
            self.pair.0.rank_name(),
            self.pair.1.rank_name(),
            self.bet.text,
            self.hands
        )
    }

    fn draw(&mut self, area: Rect, buf: &mut Buffer) {
        self.buttons.clear();
        let status = format!("purse ${}", self.purse);
        let inner = frame(
            buf,
            area,
            Self::TITLE,
            &status,
            &[
                ("Enter", "bet"),
                ("P", "pass"),
                ("↑↓", "amount"),
                ("W", "walk away"),
                ("N", "new"),
                ("Tab", "rules"),
                ("M", "sound"),
                ("Esc", "quit"),
            ],
        );
        if too_small(buf, inner, 76, 22) {
            return;
        }
        let side_w = 30u16.min(inner.width / 3 + 4);
        let table = Rect {
            width: inner.width - side_w,
            ..inner
        };
        let side = Rect {
            x: inner.x + table.width,
            width: side_w,
            ..inner
        };

        // the table
        let felt = Color::Rgb(16, 52, 34);
        let t = panel(buf, table, "The table", AMBER);
        fill(buf, t, on(INK, felt));
        let cw = 9u16;
        let gap = 4u16;
        let total = cw * 3 + gap * 2;
        let cx = t.x + t.width.saturating_sub(total) / 2;
        let cy = t.y + t.height.saturating_sub(19) / 2;
        draw_card(buf, cx, cy, Some(self.pair.0));
        let mid = match self.phase {
            Phase::Betting => None,
            Phase::Turning if self.turn_clock < 0.35 => None,
            _ => self.third,
        };
        draw_card(buf, cx + cw + gap, cy, mid);
        draw_card(buf, cx + (cw + gap) * 2, cy, Some(self.pair.1));
        put(buf, cx + cw + 1, cy + 3, "◂", on(DIM, felt));
        put(buf, cx + cw + gap + cw + 2, cy + 3, "▸", on(DIM, felt));

        let between_n = self.pair.1.rank - self.pair.0.rank - 1;
        let info_y = cy + 8;
        let odds = chance(self.pair.0, self.pair.1, &self.shoe.cards);
        if self.phase == Phase::Betting {
            centre(
                buf,
                t,
                info_y,
                &format!(
                    "{between_n} rank{} between: {:.0}% of the cards left win",
                    if between_n == 1 { "" } else { "s" },
                    odds * 100.0
                ),
                on(DIM, felt),
            );
        }

        // the ranks that win, lit along a ruler
        let ruler_y = info_y + 1;
        let rx = t.x + t.width.saturating_sub(13 * 3) / 2;
        for r in 2u8..=14 {
            let c = Card { rank: r, suit: 0 };
            let win = r > self.pair.0.rank && r < self.pair.1.rank;
            let edge = r == self.pair.0.rank || r == self.pair.1.rank;
            let fg = if win {
                GREEN
            } else if edge {
                AMBER
            } else {
                Color::Rgb(60, 90, 70)
            };
            let x = rx + (r as u16 - 2) * 3;
            let hit_now = self.third.is_some_and(|t3| t3.rank == r)
                && !matches!(self.phase, Phase::Betting)
                && !(self.phase == Phase::Turning && self.turn_clock < 0.35);
            let style = if hit_now {
                bold(on(BG, fg))
            } else {
                bold(on(fg, felt))
            };
            put(buf, x, ruler_y, &format!("{:>2}", c.rank_name()), style);
        }

        // the bet, or what happens next
        let by = ruler_y + 2;
        match self.phase {
            Phase::Betting => {
                let bx = t.x + t.width.saturating_sub(40) / 2;
                self.bet.draw(buf, bx + 2, by, 10, 10, true);
                let mut x = bx + 2;
                let row = by + 2;
                for (label, k) in [
                    (" Bet ", Key::Enter),
                    (" Pass ", Key::Char('p')),
                    (" Half ", Key::Char('h')),
                    (" All in ", Key::Char('a')),
                ] {
                    let w = label.chars().count() as u16;
                    let r = Rect::new(x, row, w, 1);
                    let (fg, bg) = if k == Key::Enter {
                        (BG, AMBER)
                    } else {
                        (INK, PAPER)
                    };
                    fill(buf, r, on(fg, bg));
                    put(buf, x, row, label, bold(on(fg, bg)));
                    self.buttons.push((r, k));
                    x += w + 2;
                }
            }
            Phase::Shown | Phase::Broke | Phase::WalkedAway => {
                let label = if self.phase == Phase::Shown {
                    " Deal again "
                } else {
                    " Sit down again "
                };
                let w = label.chars().count() as u16;
                let x = t.x + t.width.saturating_sub(w) / 2;
                let r = Rect::new(x, by + 2, w, 1);
                fill(buf, r, on(BG, AMBER));
                put(buf, x, by + 2, label, bold(on(BG, AMBER)));
                self.buttons.push((r, Key::Enter));
                if self.phase == Phase::Shown {
                    let label = " Walk away ";
                    let w2 = label.chars().count() as u16;
                    let r2 = Rect::new(x + (w.saturating_sub(w2)) / 2, by + 4, w2, 1);
                    if r2.bottom() <= t.bottom() {
                        fill(buf, r2, on(INK, PAPER));
                        put(buf, r2.x, r2.y, label, bold(on(INK, PAPER)));
                        self.buttons.push((r2, Key::Char('w')));
                    }
                }
            }
            Phase::Turning => {}
        }
        let note_r = Rect::new(t.x + 1, t.bottom().saturating_sub(2), t.width - 2, 2);
        let colour = match self.phase {
            Phase::Broke => RED,
            Phase::WalkedAway => GREEN,
            _ => AMBER,
        };
        let lines = wrap(&self.note, note_r.width as usize);
        for (i, l) in lines.iter().take(2).enumerate() {
            centre(buf, note_r, note_r.y + i as u16, l, bold(on(colour, felt)));
        }

        // the purse and the record, or the rules
        let top_h = 7u16.min(side.height);
        let p = panel(
            buf,
            Rect {
                height: top_h,
                ..side
            },
            "Purse",
            GREEN,
        );
        put(
            buf,
            p.x,
            p.y,
            &format!("${}", self.purse),
            bold(st(if self.purse >= START_PURSE {
                GREEN
            } else {
                RED
            })),
        );
        bar(
            buf,
            p.x,
            p.y + 1,
            p.width,
            self.purse as f32 / (START_PURSE as f32 * 3.0),
            GREEN,
        );
        put(
            buf,
            p.x,
            p.y + 2,
            &format!("hands  {}", self.hands),
            st(DIM),
        );
        put(
            buf,
            p.x,
            p.y + 3,
            &format!("peak   ${}", self.peak),
            st(DIM),
        );
        let rest = Rect {
            y: side.y + top_h,
            height: side.height - top_h,
            ..side
        };
        if self.show_help {
            let p = panel(buf, rest, "How it works", CYAN);
            para(
                buf,
                p,
                "Two cards face up. Bet that the next falls strictly between them; aces are high. A card that matches either one loses. Bet nothing to pass. Pairs too close to bet on are dealt again.\n\nYou win what you bet, or lose it.",
                st(INK),
            );
        } else {
            let half = rest.height / 2;
            let p = panel(
                buf,
                Rect {
                    height: rest.height - half,
                    ..rest
                },
                "Hands",
                AMBER,
            );
            let rows = p.height as usize;
            let skip = self.log.len().saturating_sub(rows);
            for (i, (l, c)) in self.log.iter().skip(skip).enumerate() {
                put_max(buf, p.x, p.y + i as u16, l, st(*c), p.width);
            }
            let p = panel(
                buf,
                Rect {
                    y: rest.y + rest.height - half,
                    height: half,
                    ..rest
                },
                "Your record",
                CYAN,
            );
            let best = self
                .stats
                .get::<i64>("best")
                .map_or("-".into(), |b| format!("${b}"));
            let lines = [
                format!("sat down   {}", self.stats.get::<u32>("games").unwrap_or(0)),
                format!("went broke {}", self.stats.get::<u32>("broke").unwrap_or(0)),
                format!("best exit  {best}"),
                format!(
                    "most held  {}",
                    self.stats
                        .get::<i64>("peak")
                        .map_or("-".into(), |b| format!("${b}"))
                ),
            ];
            for (i, l) in lines.iter().enumerate() {
                if p.y + (i as u16) < p.bottom() {
                    put(buf, p.x, p.y + i as u16, l, st(INK));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(rank: u8) -> Card {
        Card { rank, suit: 0 }
    }

    #[test]
    fn strictly_between_and_aces_high() {
        assert!(between(c(4), c(9), c(5)));
        assert!(!between(c(4), c(9), c(4)));
        assert!(!between(c(4), c(9), c(9)));
        assert!(!between(c(4), c(9), c(12)));
        assert!(between(c(10), c(14), c(13)));
        assert!(between(c(14), c(2), c(7)));
    }

    #[test]
    fn pairs_always_leave_a_gap() {
        let mut rng = Rng::new(3);
        let mut shoe = Shoe::new(&mut rng);
        for _ in 0..200 {
            let (a, b) = deal_pair(&mut shoe, &mut rng);
            assert!(b.rank >= a.rank + 2);
        }
    }

    #[test]
    fn chance_counts_the_cards_left() {
        let mut rng = Rng::new(5);
        let shoe = Shoe::new(&mut rng);
        // 2 and ace: every 3 to king wins, 44 of 52
        let p = chance(c(2), c(14), &shoe.cards);
        assert!((p - 44.0 / 52.0).abs() < 1e-6);
    }
}
