//! Fur Trader: eight seasons as an independent trader on Lake Ontario in
//! the 1700s. Outfit a canoe with trade goods and voyageurs, winter in the
//! upper country trading with the trappers of the nations there, and each
//! spring choose where to sell. Rules in asciiverse/specs/fur-trader.md.

use crate::audio::Sound;
use crate::rng::Rng;
use crate::shell::{Ctx, Game, Key, Mouse, MouseKind};
use crate::store::Kv;
use crate::ui::*;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

pub const SEASONS: u32 = 8;
pub const START_MONEY: i64 = 600;
/// livres a voyageur is paid for a season
pub const WAGE: i64 = 25;
/// bales one voyageur can carry and trade
pub const BALES_PER_MAN: i64 = 3;
pub const MIN_CREW: i64 = 2;
pub const MAX_CREW: i64 = 12;

pub const FURS: [&str; 5] = ["Beaver", "Otter", "Marten", "Fox", "Mink"];
/// livres a pelt, before the market and the post
const BASE_PRICE: [f32; 5] = [6.0, 9.0, 4.0, 5.0, 3.0];
/// how a winter's pelts divide among the kinds
const SHARE: [f32; 5] = [0.5, 0.1, 0.2, 0.12, 0.08];

pub struct Post {
    pub name: &'static str,
    pub route: &'static str,
    pub about: &'static str,
    /// prices against the base
    pub pays: f32,
    /// livres a bale of trade goods costs when you outfit after selling here
    pub goods: i64,
    /// the path on the map, in fractions of its width and height
    pub path: &'static [(f32, f32)],
}

const CAMP: (f32, f32) = (0.10, 0.30);
const FRONTENAC: (f32, f32) = (0.66, 0.30);
const MONTREAL: (f32, f32) = (0.95, 0.06);
const OSWEGO: (f32, f32) = (0.50, 0.56);
const CARRY: (f32, f32) = (0.64, 0.70);
const ALBANY: (f32, f32) = (0.93, 0.90);

pub const POSTS: [Post; 3] = [
    Post {
        name: "Fort Frontenac",
        route: "along the lake to its eastern end",
        about: "The French fort where the lake empties into the St. Lawrence. Close and safe, but the King's storekeeper pays the least.",
        pays: 0.85,
        goods: 55,
        path: &[CAMP, (0.38, 0.24), FRONTENAC],
    },
    Post {
        name: "Montreal",
        route: "down the St. Lawrence and its rapids",
        about: "The great market of New France. Good prices, but the river runs through long rapids, and a canoe can spill.",
        pays: 1.10,
        goods: 50,
        path: &[CAMP, (0.38, 0.24), FRONTENAC, (0.80, 0.18), MONTREAL],
    },
    Post {
        name: "Albany",
        route: "by Oswego, the Great Carrying Place and the Mohawk",
        about: "The English town on the Hudson. The best prices and the cheapest cloth, but the longest way, with a hard carry where pelts get wet.",
        pays: 1.20,
        goods: 40,
        path: &[CAMP, (0.36, 0.45), OSWEGO, CARRY, (0.80, 0.78), ALBANY],
    },
];

const PARTNERS: [&str; 4] = [
    "Anishinaabe families at their winter camps",
    "Wendat traders at their village",
    "Haudenosaunee hunters coming in from the hills",
    "Odawa traders who came down by canoe",
];

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Phase {
    Outfit,
    Winter,
    Choose,
    Journey,
    Sale,
    Over,
}

/// What a spring's journey and sale came to.
#[derive(Clone, Debug, Default)]
pub struct Sale {
    pub post: usize,
    pub prices: [f32; 5],
    pub sold: [i64; 5],
    pub total: i64,
    pub events: Vec<(String, bool)>,
    pub extra_wages: i64,
}

#[derive(Clone, Debug)]
pub struct Trader {
    pub season: u32,
    pub money: i64,
    pub bales: i64,
    pub crew: i64,
    pub pelts: [i64; 5],
    /// the post last sold at: it sets the price of next season's goods
    pub last_post: usize,
    /// the market in Europe for beaver, set each spring
    pub beaver_market: f32,
    pub history: Vec<i64>,
}

impl Trader {
    pub fn new() -> Trader {
        Trader {
            season: 1,
            money: START_MONEY,
            bales: 0,
            crew: 0,
            pelts: [0; 5],
            last_post: 1,
            beaver_market: 1.0,
            history: vec![START_MONEY],
        }
    }

    pub fn goods_price(&self) -> i64 {
        POSTS[self.last_post].goods
    }

    pub fn outfit_cost(&self, bales: i64, crew: i64) -> i64 {
        bales * self.goods_price() + crew * WAGE
    }

    pub fn check_outfit(&self, bales: i64, crew: i64) -> Option<String> {
        if !(MIN_CREW..=MAX_CREW).contains(&crew) {
            return Some(format!("A canoe needs {MIN_CREW} to {MAX_CREW} voyageurs."));
        }
        if bales < 0 {
            return Some("Bales can't be negative.".into());
        }
        if bales > crew * BALES_PER_MAN {
            return Some(format!(
                "{crew} voyageurs carry at most {} bales.",
                crew * BALES_PER_MAN
            ));
        }
        let cost = self.outfit_cost(bales, crew);
        if cost > self.money {
            return Some(format!(
                "That costs {cost} livres; you have {}.",
                self.money
            ));
        }
        None
    }

    /// The most bales the purse and a full crew allow.
    pub fn best_outfit(&self) -> (i64, i64) {
        let mut best = (0, MIN_CREW);
        for crew in MIN_CREW..=MAX_CREW {
            let afford = (self.money - crew * WAGE) / self.goods_price();
            let bales = afford.min(crew * BALES_PER_MAN);
            if bales > best.0 {
                best = (bales, crew);
            }
        }
        best
    }

    pub fn outfit(&mut self, bales: i64, crew: i64) {
        self.money -= self.outfit_cost(bales, crew);
        self.bales = bales;
        self.crew = crew;
    }

    /// The winter's trading: goods for pelts. Returns the story.
    pub fn winter(&mut self, rng: &mut Rng) -> Vec<(String, bool)> {
        let mut out = Vec::new();
        let partner = PARTNERS[rng.range(0, PARTNERS.len() as i64 - 1) as usize];
        out.push((
            format!("You winter in the upper country and trade with {partner}."),
            true,
        ));
        let mut per_bale = rng.between(10.0, 16.0);
        if rng.chance(0.15) {
            per_bale *= 0.7;
            out.push((
                "A hard winter with deep snow: the trappers bring in fewer pelts.".into(),
                false,
            ));
        } else if rng.chance(0.15) {
            per_bale *= 1.2;
            out.push((
                "A good winter for hunting: more pelts than you hoped for.".into(),
                true,
            ));
        }
        let total = self.bales as f32 * per_bale;
        for (i, s) in SHARE.iter().enumerate() {
            self.pelts[i] = (total * s * rng.between(0.8, 1.2)).round() as i64;
        }
        self.bales = 0;
        out.push((
            format!(
                "Your goods brought {} pelts.",
                self.pelts.iter().sum::<i64>()
            ),
            true,
        ));
        out
    }

    /// The journey to a post and the sale there.
    pub fn sell_at(&mut self, post: usize, rng: &mut Rng) -> Sale {
        let mut s = Sale {
            post,
            ..Sale::default()
        };
        let mut prices_by = 1.0;
        let lose = |pelts: &mut [i64; 5], frac: f32| {
            let mut lost = 0;
            for p in pelts.iter_mut() {
                let l = (*p as f32 * frac).round() as i64;
                *p -= l;
                lost += l;
            }
            lost
        };
        if rng.chance(0.12) {
            s.extra_wages = self.crew * 5;
            s.events.push((
                format!(
                    "A storm on the lake keeps you ashore for days: {} livres more in wages.",
                    s.extra_wages
                ),
                false,
            ));
        }
        match post {
            1 => {
                if rng.chance(0.30) {
                    let lost = lose(&mut self.pelts, rng.between(0.10, 0.30));
                    s.events.push((
                        format!("A canoe takes water in the rapids: {lost} pelts are lost to the river."),
                        false,
                    ));
                }
            }
            2 => {
                if rng.chance(0.20) {
                    s.events.push((
                        "A Mohawk guide shows your crew a drier path over the carrying place."
                            .into(),
                        true,
                    ));
                } else if rng.chance(0.30) {
                    let lost = lose(&mut self.pelts, rng.between(0.15, 0.30));
                    s.events.push((
                        format!("Rain on the long carry: {lost} pelts are spoiled by the wet."),
                        false,
                    ));
                }
                if rng.chance(0.25) {
                    prices_by *= 0.85;
                    s.events.push((
                        "A rival trader reached Albany first; the buyers are choosy.".into(),
                        false,
                    ));
                }
            }
            _ => {}
        }
        if rng.chance(0.04) {
            let lost = lose(&mut self.pelts, 0.33);
            s.events.push((
                format!("A canoe overturns in a squall. The crew swim ashore safe, but {lost} pelts are gone."),
                false,
            ));
        }
        self.beaver_market = rng.between(0.8, 1.25);
        if self.beaver_market > 1.12 {
            s.events.push((
                "Ships from Europe bring word: beaver hats are in fashion, and beaver is dear."
                    .into(),
                true,
            ));
        } else if self.beaver_market < 0.9 {
            s.events.push((
                "Ships from Europe bring word: the warehouses there are full of beaver.".into(),
                false,
            ));
        }
        for i in 0..5 {
            let market = if i == 0 { self.beaver_market } else { 1.0 };
            s.prices[i] = (BASE_PRICE[i]
                * POSTS[post].pays
                * market
                * prices_by
                * rng.between(0.9, 1.1)
                * 10.0)
                .round()
                / 10.0;
            s.sold[i] = self.pelts[i];
            s.total += (self.pelts[i] as f32 * s.prices[i]).round() as i64;
            self.pelts[i] = 0;
        }
        self.money += s.total - s.extra_wages;
        self.money = self.money.max(0);
        self.last_post = post;
        self.history.push(self.money);
        s
    }

    /// After the sale: on to the next season, or out of the trade.
    pub fn next_season(&mut self) {
        self.season += 1;
    }

    pub fn over(&self) -> bool {
        self.season > SEASONS || (self.money < MIN_CREW * WAGE + self.goods_price())
    }

    /// The demo's (and a cautious player's) pick: weigh price against risk.
    pub fn pick(&self, rng: &mut Rng) -> usize {
        let r = rng.unit();
        if r < 0.25 {
            0
        } else if r < 0.6 {
            1
        } else {
            2
        }
    }
}

impl Default for Trader {
    fn default() -> Self {
        Trader::new()
    }
}

pub struct FurTrader {
    ctx: Ctx,
    t: Trader,
    phase: Phase,
    fields: [NumField; 2],
    focus: usize,
    fresh: bool,
    choice: usize,
    sale: Option<Sale>,
    /// 0 to 1 along the route while travelling
    progress: f32,
    log: Vec<(String, Color)>,
    note: String,
    show_help: bool,
    record: Kv,
    quit: bool,
    demo_clock: f32,
    buttons: Vec<(Rect, Key)>,
}

impl FurTrader {
    fn say(&mut self, s: impl Into<String>, c: Color) {
        self.log.push((s.into(), c));
        if self.log.len() > 60 {
            self.log.remove(0);
        }
    }

    fn new_game(&mut self) {
        self.t = Trader::new();
        self.log.clear();
        self.sale = None;
        self.say(
            "You set up as a trader on the north shore of Lake Ontario, with 600 livres.",
            INK,
        );
        self.start_outfit();
        log::info!("new game");
    }

    fn start_outfit(&mut self) {
        self.phase = Phase::Outfit;
        let (b, c) = self.t.best_outfit();
        self.fields = [
            NumField::new("Bales of trade goods", b, 0, 999, 1),
            NumField::new("Voyageurs to hire", c, MIN_CREW, MAX_CREW, 1),
        ];
        self.focus = 0;
        self.fresh = true;
        self.say(
            format!(
                "Autumn of season {}. Goods are {} livres a bale, a voyageur {WAGE}.",
                self.t.season,
                self.t.goods_price()
            ),
            CYAN,
        );
    }

    fn set_out(&mut self) {
        let (b, c) = (self.fields[0].value(), self.fields[1].value());
        if let Some(why) = self.t.check_outfit(b, c) {
            self.note = why;
            self.ctx.audio.play(Sound::Nope);
            return;
        }
        self.note.clear();
        self.t.outfit(b, c);
        log::info!("outfit {b} bales {c} crew, money {}", self.t.money);
        for (s, good) in self.t.winter(&mut self.ctx.rng) {
            self.say(s, if good { INK } else { AMBER });
        }
        self.phase = Phase::Winter;
        self.ctx.audio.play(Sound::Click);
    }

    fn choose(&mut self) {
        self.phase = Phase::Choose;
        self.say("Spring. The ice is out. Where will you sell?", CYAN);
    }

    fn depart(&mut self) {
        self.phase = Phase::Journey;
        self.progress = 0.0;
        let p = &POSTS[self.choice];
        self.say(format!("You set off for {}, {}.", p.name, p.route), INK);
        log::info!("depart for {}", p.name);
        self.ctx.audio.play(Sound::Click);
    }

    fn arrive(&mut self) {
        let s = self.t.sell_at(self.choice, &mut self.ctx.rng);
        for (e, good) in &s.events {
            self.say(e.clone(), if *good { GREEN } else { AMBER });
        }
        self.say(
            format!(
                "At {} your pelts fetch {} livres.",
                POSTS[s.post].name, s.total
            ),
            GREEN,
        );
        log::info!(
            "sold at {} for {} -> money {}",
            POSTS[s.post].name,
            s.total,
            self.t.money
        );
        self.ctx.audio.play(if s.events.iter().any(|e| !e.1) {
            Sound::Bad
        } else {
            Sound::Good
        });
        self.sale = Some(s);
        self.phase = Phase::Sale;
    }

    fn after_sale(&mut self) {
        self.t.next_season();
        if self.t.over() {
            self.phase = Phase::Over;
            let won = self.t.money > START_MONEY;
            self.ctx
                .audio
                .play(if won { Sound::Win } else { Sound::Lose });
            self.say(
                format!("You retire from the trade with {} livres.", self.t.money),
                if won { GREEN } else { RED },
            );
            if !self.ctx.demo {
                let games = self.record.get::<u32>("games").unwrap_or(0) + 1;
                self.record.set("games", games);
                if self.t.money > self.record.get::<i64>("best").unwrap_or(0) {
                    self.record.set("best", self.t.money);
                }
                self.record.save(Self::ID, "record");
            }
        } else {
            self.start_outfit();
        }
    }

    fn advance(&mut self) {
        match self.phase {
            Phase::Outfit => self.set_out(),
            Phase::Winter => self.choose(),
            Phase::Choose => self.depart(),
            Phase::Journey => {}
            Phase::Sale => self.after_sale(),
            Phase::Over => self.new_game(),
        }
    }

    fn draw_map(&self, buf: &mut Buffer, r: Rect) {
        let p = panel(buf, r, "Lake Ontario and the ways to market", CYAN);
        if p.width < 20 || p.height < 6 {
            return;
        }
        let at = |(fx, fy): (f32, f32)| -> (u16, u16) {
            (
                p.x + (fx * (p.width - 1) as f32).round() as u16,
                p.y + (fy * (p.height - 1) as f32).round() as u16,
            )
        };
        let land = Color::Rgb(20, 28, 20);
        fill(buf, p, on(Color::Rgb(60, 80, 52), land));
        for y in p.top()..p.bottom() {
            for x in p.left()..p.right() {
                if (x * 7 + y * 13) % 11 == 0 {
                    buf[(x, y)]
                        .set_symbol("^")
                        .set_style(on(Color::Rgb(46, 70, 44), land));
                }
            }
        }
        // the lake
        let (cx, cy) = (0.38 * (p.width - 1) as f32, 0.36 * (p.height - 1) as f32);
        let (rx, ry) = (0.30 * p.width as f32, 0.19 * p.height as f32 + 0.6);
        let water = Color::Rgb(24, 52, 86);
        for y in p.top()..p.bottom() {
            for x in p.left()..p.right() {
                let dx = (x - p.x) as f32 - cx;
                let dy = (y - p.y) as f32 - cy;
                if (dx / rx).powi(2) + (dy / ry).powi(2) <= 1.0 {
                    let s = if (x + y) % 4 == 0 { "~" } else { " " };
                    buf[(x, y)]
                        .set_symbol(s)
                        .set_style(on(Color::Rgb(80, 120, 170), water));
                }
            }
        }
        // rivers: the St. Lawrence and the Mohawk, with the Hudson
        let mut line = |a: (f32, f32), b: (f32, f32), sym: &str, fg: Color| {
            let (ax, ay) = at(a);
            let (bx, by) = at(b);
            let n = (ax.abs_diff(bx).max(ay.abs_diff(by)) * 2).max(1);
            for i in 0..=n {
                let k = i as f32 / n as f32;
                let x = (ax as f32 + (bx as f32 - ax as f32) * k).round() as u16;
                let y = (ay as f32 + (by as f32 - ay as f32) * k).round() as u16;
                if hit(&p, x, y) {
                    buf[(x, y)].set_symbol(sym).set_style(on(fg, land));
                }
            }
        };
        let river = Color::Rgb(90, 140, 200);
        line(FRONTENAC, (0.80, 0.18), "≈", river);
        line((0.80, 0.18), MONTREAL, "≈", river);
        line(OSWEGO, CARRY, "·", river);
        line(CARRY, (0.80, 0.78), "≈", river);
        line((0.80, 0.78), ALBANY, "≈", river);
        line(ALBANY, (0.95, 0.99), "≈", river);
        // the chosen route
        let shown = match self.phase {
            Phase::Choose | Phase::Journey | Phase::Sale => Some(self.choice),
            _ => None,
        };
        if let Some(i) = shown {
            let path = POSTS[i].path;
            for w in path.windows(2) {
                line(w[0], w[1], "•", AMBER);
            }
        }
        // places
        let mut place = |pt: (f32, f32), name: &str, fg: Color, left: bool| {
            let (x, y) = at(pt);
            if hit(&p, x, y) {
                buf[(x, y)].set_symbol("■").set_style(bold(on(fg, land)));
            }
            let w = name.chars().count() as u16;
            let lx = if left {
                x.saturating_sub(w + 1).max(p.x)
            } else {
                (x + 2).min(p.right().saturating_sub(w))
            };
            put_max(buf, lx, y, name, bold(on(fg, land)), p.right() - lx);
        };
        place(CAMP, "camp", GREEN, false);
        place(FRONTENAC, "Frontenac", INK, false);
        place(MONTREAL, "Montreal", INK, true);
        place(OSWEGO, "Oswego", DIM, true);
        place(ALBANY, "Albany", INK, true);
        // the canoe
        if self.phase == Phase::Journey {
            let path = POSTS[self.choice].path;
            let segs = path.len() - 1;
            let k = (self.progress * segs as f32).min(segs as f32 - 0.001);
            let i = k as usize;
            let f = k - i as f32;
            let (a, b) = (path[i], path[i + 1]);
            let (x, y) = at((a.0 + (b.0 - a.0) * f, a.1 + (b.1 - a.1) * f));
            if hit(&p, x, y) {
                buf[(x, y)].set_symbol("◆").set_style(bold(on(AMBER, BG)));
            }
        }
    }

    /// Money at the start and after each spring's sale, as bars.
    fn draw_fortune(&self, buf: &mut Buffer, r: Rect) {
        let p = panel(buf, r, "Fortune by season", GREEN);
        if p.height < 1 || p.width < 16 {
            return;
        }
        let max = self.t.history.iter().copied().max().unwrap_or(1).max(1) as f32;
        let rows = p.height as usize;
        let skip = self.t.history.len().saturating_sub(rows);
        for (i, m) in self.t.history.iter().enumerate().skip(skip) {
            let y = p.y + (i - skip) as u16;
            let label = if i == 0 {
                "start".to_string()
            } else {
                format!("{i:>5}")
            };
            put(buf, p.x, y, &label, st(DIM));
            let w = p.width.saturating_sub(14);
            bar(buf, p.x + 6, y, w, *m as f32 / max, GREEN);
            put(buf, p.x + 7 + w, y, &format!("{m:>6}"), st(INK));
        }
    }

    fn draw_ledger(&self, buf: &mut Buffer, r: Rect) {
        let p = panel(buf, r, "Ledger", AMBER);
        let t = &self.t;
        let mut lines: Vec<(String, Color)> = vec![
            (format!("Livres     {}", t.money), AMBER),
            (
                format!("Goods      {} bales, {} voyageurs", t.bales, t.crew),
                INK,
            ),
        ];
        let pelts: i64 = t.pelts.iter().sum();
        if pelts > 0 {
            let s: Vec<String> = FURS
                .iter()
                .zip(t.pelts.iter())
                .map(|(f, n)| format!("{}{n}", &f[..2]))
                .collect();
            lines.push((format!("Pelts      {pelts}"), GREEN));
            lines.push((format!("           {}", s.join(" ")), GREEN));
        } else {
            lines.push(("Pelts      none".into(), DIM));
        }
        for (i, (l, c)) in lines.iter().enumerate() {
            if p.y + (i as u16) < p.bottom() {
                put_max(buf, p.x, p.y + i as u16, l, st(*c), p.width);
            }
        }
    }

    fn draw_action(&mut self, buf: &mut Buffer, r: Rect) {
        let title = match self.phase {
            Phase::Outfit => "Outfit the canoe",
            Phase::Winter => "Winter trade",
            Phase::Choose => "Where to sell",
            Phase::Journey => "On the way",
            Phase::Sale => "The sale",
            Phase::Over => "Retired",
        };
        let p = panel(buf, r, title, AMBER);
        if p.height < 3 {
            return;
        }
        match self.phase {
            Phase::Outfit => {
                for (i, f) in self.fields.iter().enumerate() {
                    let y = p.y + i as u16 * 2;
                    put(
                        buf,
                        p.x + 2,
                        y,
                        f.label,
                        st(if i == self.focus { INK } else { DIM }),
                    );
                    f.draw(buf, p.x + 2, y + 1, 0, 10, i == self.focus);
                    self.buttons.push((
                        Rect::new(p.x, y, p.width, 2),
                        Key::Char(if i == 0 { 'B' } else { 'V' }),
                    ));
                }
                let (b, c) = (self.fields[0].value(), self.fields[1].value());
                let y = p.y + 4;
                let (msg, col) = match self.t.check_outfit(b, c) {
                    Some(why) => (why, RED),
                    None if !self.note.is_empty() => (self.note.clone(), RED),
                    None => (
                        format!(
                            "Costs {}, leaves {} livres.",
                            self.t.outfit_cost(b, c),
                            self.t.money - self.t.outfit_cost(b, c)
                        ),
                        GREEN,
                    ),
                };
                para(buf, Rect::new(p.x, y + 1, p.width, 2), &msg, st(col));
                para(
                    buf,
                    Rect::new(p.x, y + 3, p.width, p.bottom().saturating_sub(y + 3)),
                    &format!("Each voyageur carries {BALES_PER_MAN} bales. Enter sets out."),
                    st(DIM),
                );
            }
            Phase::Choose => {
                let items: Vec<String> = POSTS
                    .iter()
                    .enumerate()
                    .map(|(i, q)| format!("{} {}", i + 1, q.name))
                    .collect();
                let rows = menu(buf, Rect { height: 3, ..p }, &items, self.choice);
                for (i, row) in rows.into_iter().enumerate() {
                    self.buttons
                        .push((row, Key::Char((b'1' + i as u8) as char)));
                }
                let q = &POSTS[self.choice];
                para(
                    buf,
                    Rect::new(p.x, p.y + 4, p.width, p.height.saturating_sub(4)),
                    &format!("{} {}", q.about, ""),
                    st(INK),
                );
            }
            Phase::Sale => {
                if let Some(s) = &self.sale {
                    put(
                        buf,
                        p.x,
                        p.y,
                        &format!("At {}", POSTS[s.post].name),
                        bold(st(INK)),
                    );
                    for i in 0..5 {
                        let y = p.y + 1 + i as u16;
                        if y >= p.bottom() {
                            break;
                        }
                        let line = format!(
                            "{:<7}{:>4} at {:>4.1} = {:>5}",
                            FURS[i],
                            s.sold[i],
                            s.prices[i],
                            (s.sold[i] as f32 * s.prices[i]).round() as i64
                        );
                        put_max(buf, p.x, y, &line, st(INK), p.width);
                    }
                    let y = p.y + 6;
                    if y < p.bottom() {
                        put(
                            buf,
                            p.x,
                            y,
                            &format!("In all {:>18}", s.total),
                            bold(st(GREEN)),
                        );
                    }
                }
            }
            Phase::Over => {
                let lines = [
                    format!("Your fortune: {} livres", self.t.money),
                    format!(
                        "Best: {}",
                        self.record
                            .get::<i64>("best")
                            .map_or("-".into(), |b| b.to_string())
                    ),
                    "Enter for a new life in the trade.".into(),
                ];
                for (i, l) in lines.iter().enumerate() {
                    put_max(
                        buf,
                        p.x,
                        p.y + i as u16,
                        l,
                        st(if i == 0 { GREEN } else { DIM }),
                        p.width,
                    );
                }
            }
            Phase::Winter => {
                para(
                    buf,
                    p,
                    "The pelts are baled and the canoe is ready for spring. Enter goes on.",
                    st(INK),
                );
            }
            Phase::Journey => {
                bar(buf, p.x, p.y, p.width, self.progress, AMBER);
                para(
                    buf,
                    Rect::new(p.x, p.y + 2, p.width, p.height.saturating_sub(2)),
                    &format!(
                        "To {}, {}.",
                        POSTS[self.choice].name, POSTS[self.choice].route
                    ),
                    st(DIM),
                );
            }
        }
    }
}

impl Game for FurTrader {
    const ID: &'static str = "fur-trader";
    const TITLE: &'static str = "Fur Trader";
    const HELP: &'static str =
        "Eight seasons as an independent fur trader on Lake Ontario in the 1700s.
Each autumn buy bales of trade goods and hire voyageurs (each carries 3
bales, for 25 livres). Over the winter your goods are traded for pelts:
beaver, otter, marten, fox and mink. In spring choose where to sell:
Fort Frontenac is near and safe but pays least; Montreal pays well past
the St. Lawrence rapids; Albany pays best and sells cheap goods, by the
longest way. End with more than you began.
Tab moves between fields, Up/Down or typing changes them, 1-3 or the
arrows choose a post, Enter goes on.";

    fn new(ctx: Ctx) -> Self {
        let mut g = FurTrader {
            ctx,
            t: Trader::new(),
            phase: Phase::Outfit,
            fields: [NumField::new("", 0, 0, 0, 1), NumField::new("", 0, 0, 0, 1)],
            focus: 0,
            fresh: true,
            choice: 1,
            sale: None,
            progress: 0.0,
            log: Vec::new(),
            note: String::new(),
            show_help: false,
            record: Kv::load("fur-trader", "record"),
            quit: false,
            demo_clock: 0.0,
            buttons: Vec::new(),
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
            Key::Char('m') => {
                self.ctx.audio.toggle();
                crate::shell::remember_mute(Self::ID, self.ctx.audio.muted);
            }
            Key::Char('?') => self.show_help = !self.show_help,
            Key::Enter => self.advance(),
            // the mouse picks a field with these
            Key::Char('B') => {
                self.focus = 0;
                self.fresh = true;
            }
            Key::Char('V') => {
                self.focus = 1;
                self.fresh = true;
            }
            k if self.phase == Phase::Choose => match k {
                Key::Up | Key::Left => self.choice = (self.choice + 2) % 3,
                Key::Down | Key::Right => self.choice = (self.choice + 1) % 3,
                Key::Char(c @ '1'..='3') => {
                    let i = c as usize - '1' as usize;
                    if self.choice == i {
                        self.depart();
                    } else {
                        self.choice = i;
                        self.ctx.audio.play(Sound::Tick);
                    }
                }
                Key::Tab => self.show_help = !self.show_help,
                _ => {}
            },
            k if self.phase == Phase::Outfit => match k {
                Key::Tab | Key::BackTab | Key::Left | Key::Right => {
                    self.focus = 1 - self.focus;
                    self.fresh = true;
                    self.ctx.audio.play(Sound::Tick);
                }
                other => {
                    let mut fresh = self.fresh;
                    if self.fields[self.focus].key(&other, &mut fresh) {
                        self.fresh = fresh;
                    }
                }
            },
            Key::Tab => self.show_help = !self.show_help,
            _ => {}
        }
    }

    fn mouse(&mut self, m: Mouse) {
        match m.kind {
            MouseKind::Click => {
                if let Some((_, k)) = self.buttons.iter().find(|(r, _)| hit(r, m.x, m.y)).cloned() {
                    self.key(k);
                }
            }
            MouseKind::ScrollUp if self.phase == Phase::Outfit => {
                let mut f = true;
                self.fields[self.focus].key(&Key::Up, &mut f);
            }
            MouseKind::ScrollDown if self.phase == Phase::Outfit => {
                let mut f = true;
                self.fields[self.focus].key(&Key::Down, &mut f);
            }
            _ => {}
        }
    }

    fn tick(&mut self, dt: f32) {
        if self.phase == Phase::Journey {
            self.progress += dt / 3.0;
            if self.progress >= 1.0 {
                self.progress = 1.0;
                self.arrive();
            }
            return;
        }
        if !self.ctx.demo {
            return;
        }
        self.demo_clock += dt;
        let wait = match self.phase {
            Phase::Outfit => 1.6,
            Phase::Winter => 2.2,
            Phase::Choose => 1.8,
            Phase::Sale => 3.0,
            Phase::Over => 5.0,
            Phase::Journey => 0.0,
        };
        if self.demo_clock < wait {
            return;
        }
        self.demo_clock = 0.0;
        if self.phase == Phase::Winter {
            self.choice = self.t.pick(&mut self.ctx.rng);
        }
        self.advance();
    }

    fn quit(&self) -> bool {
        self.quit
    }

    fn snapshot(&self) -> String {
        let t = &self.t;
        format!(
            "season {} phase {:?} money {} bales {} crew {} pelts {:?} choice {} focus {}",
            t.season, self.phase, t.money, t.bales, t.crew, t.pelts, self.choice, self.focus
        )
    }

    fn draw(&mut self, area: Rect, buf: &mut Buffer) {
        self.buttons.clear();
        let status = if self.phase == Phase::Over {
            "retired".to_string()
        } else {
            format!("season {} of {SEASONS}", self.t.season.min(SEASONS))
        };
        let keys: &[(&str, &str)] = match self.phase {
            Phase::Outfit => &[
                ("Tab", "next"),
                ("↑↓", "change"),
                ("0-9", "type"),
                ("Enter", "set out"),
                ("?", "rules"),
                ("M", "sound"),
                ("Esc", "quit"),
            ],
            Phase::Choose => &[
                ("1-3 ↑↓", "post"),
                ("Enter", "go"),
                ("?", "rules"),
                ("M", "sound"),
                ("Esc", "quit"),
            ],
            _ => &[
                ("Enter", "go on"),
                ("?", "rules"),
                ("M", "sound"),
                ("Esc", "quit"),
            ],
        };
        let inner = frame(buf, area, "Fur Trader", &status, keys);
        if too_small(buf, inner, 78, 22) {
            return;
        }
        let left_w = (inner.width * 56 / 100).max(44);
        let left = Rect {
            width: left_w,
            ..inner
        };
        let right = Rect {
            x: inner.x + left_w,
            width: inner.width - left_w,
            ..inner
        };
        let map_h = (left.height * 3 / 5).max(12);
        self.draw_map(
            buf,
            Rect {
                height: map_h,
                ..left
            },
        );
        let lg = panel(
            buf,
            Rect {
                y: left.y + map_h,
                height: left.height - map_h,
                ..left
            },
            "Season log",
            AMBER,
        );
        // newest at the bottom, wrapped
        let mut rows: Vec<(String, Color)> = Vec::new();
        for (s, c) in &self.log {
            for l in wrap(s, lg.width.max(1) as usize) {
                rows.push((l, *c));
            }
        }
        let skip = rows.len().saturating_sub(lg.height as usize);
        for (i, (l, c)) in rows.iter().skip(skip).enumerate() {
            put_max(buf, lg.x, lg.y + i as u16, l, st(*c), lg.width);
        }

        let ledger_h = 6;
        self.draw_ledger(
            buf,
            Rect {
                height: ledger_h,
                ..right
            },
        );
        let rest = Rect {
            y: right.y + ledger_h,
            height: right.height - ledger_h,
            ..right
        };
        if self.show_help {
            let p = panel(buf, rest, "How the trade works", CYAN);
            para(buf, p, "Autumn: buy bales of goods (cloth, kettles, knives, beads) and hire voyageurs, 3 bales a man, 25 livres a season. Winter: your goods are traded for pelts, 10 to 16 a bale. Spring: sell at a post. Frontenac pays 85% and is safe; Montreal 110%, with rapids; Albany 120%, with a wet carry and rivals, and its goods are the cheapest. Beaver follows the market in Europe.", st(INK));
        } else {
            let action_h = rest.height.min(11);
            self.draw_action(
                buf,
                Rect {
                    height: action_h,
                    ..rest
                },
            );
            if rest.height >= action_h + 4 {
                self.draw_fortune(
                    buf,
                    Rect {
                        y: rest.y + action_h,
                        height: rest.height - action_h,
                        ..rest
                    },
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outfits_are_checked() {
        let t = Trader::new();
        assert!(t.check_outfit(9, 3).is_none());
        assert!(
            t.check_outfit(10, 3).is_some(),
            "too many bales for the crew"
        );
        assert!(t.check_outfit(9, 1).is_some(), "too small a crew");
        assert!(t.check_outfit(30, 12).is_some(), "too dear");
        let (b, c) = t.best_outfit();
        assert!(t.check_outfit(b, c).is_none());
        assert!(b >= 9);
    }

    #[test]
    fn a_season_turns_goods_into_pelts_and_pelts_into_money() {
        let mut rng = Rng::new(11);
        let mut t = Trader::new();
        let (b, c) = t.best_outfit();
        t.outfit(b, c);
        t.winter(&mut rng);
        assert!(t.pelts.iter().sum::<i64>() > 0);
        assert_eq!(t.bales, 0);
        let before = t.money;
        let s = t.sell_at(0, &mut rng);
        assert!(s.total > 0);
        assert_eq!(t.pelts, [0; 5]);
        assert!(t.money > before);
        t.next_season();
        assert_eq!(t.season, 2);
        assert_eq!(t.goods_price(), POSTS[0].goods);
    }

    #[test]
    fn trading_usually_pays() {
        let mut ahead = 0;
        for seed in 1..30 {
            let mut rng = Rng::new(seed);
            let mut t = Trader::new();
            while !t.over() {
                let (b, c) = t.best_outfit();
                t.outfit(b, c);
                t.winter(&mut rng);
                let p = t.pick(&mut rng);
                t.sell_at(p, &mut rng);
                t.next_season();
            }
            if t.money > START_MONEY {
                ahead += 1;
            }
        }
        assert!(ahead > 20, "only {ahead} of 29 came out ahead");
    }
}
