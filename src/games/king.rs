//! King: eight years as premier of Pellora, a small island nation. Feed
//! the people, plant the farms, and decide how much of the island to sell
//! to foreign industry, which pays well and fouls the air and water. Rules
//! in asciiverse/specs/king.md.

use crate::audio::Sound;
use crate::rng::Rng;
use crate::shell::{Ctx, Game, Key, Mouse, MouseKind};
use crate::store::Kv;
use crate::ui::*;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

pub const YEARS: u32 = 8;
/// crowns a citizen needs for a year's food
pub const FOOD: i64 = 80;
/// hectares one citizen can farm
pub const HA_PER_FARMER: i64 = 2;
pub const ISLAND: &str = "Pellora";

/// What one year brought, for the report and the chart.
#[derive(Clone, Debug, Default)]
pub struct Year {
    pub year: u32,
    pub sold: i64,
    pub sale_money: i64,
    pub hungry: i64,
    pub lost_hunger: i64,
    pub lost_illness: i64,
    pub left: i64,
    pub born: i64,
    pub workers_came: i64,
    pub harvest: i64,
    pub tourism: i64,
    pub levies: i64,
    pub pollution: f32,
    pub citizens: i64,
    pub treasury: i64,
    pub forest: i64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum End {
    /// too many lost in one year: the assembly votes no confidence
    NoConfidence,
    /// people went hungry while the treasury stayed full
    Hoarding,
    /// too few citizens left on the island
    Emptied,
    /// the term served; the election's result by score
    Served(u32),
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Orders {
    pub sell: i64,
    pub food: i64,
    pub plant: i64,
    pub control: i64,
}

#[derive(Clone, Debug)]
pub struct Nation {
    pub year: u32,
    pub treasury: i64,
    pub citizens: i64,
    pub start_citizens: i64,
    pub workers: i64,
    pub forest: i64,
    pub farm: i64,
    pub industry: i64,
    /// crowns a hectare, offered by industry this year
    pub price: i64,
    /// crowns to plant a hectare this year
    pub plant_cost: i64,
    pub history: Vec<Year>,
    pub over: Option<End>,
}

impl Nation {
    pub fn new(rng: &mut Rng) -> Nation {
        let citizens = 500 + rng.range(-12, 12);
        let treasury = 60_000 + rng.range(-40, 40) * 100;
        let y = Year {
            citizens,
            treasury,
            forest: 1000,
            ..Year::default()
        };
        Nation {
            year: 1,
            treasury,
            citizens,
            start_citizens: citizens,
            workers: 0,
            forest: 1000,
            farm: 1000,
            industry: 0,
            price: rng.range(90, 120),
            plant_cost: rng.range(10, 15),
            history: vec![y],
            over: None,
        }
    }

    pub fn land(&self) -> i64 {
        self.forest + self.farm
    }

    /// Farmland and forest each give up half of a sale (farmland first
    /// when it is odd, or when one runs out).
    fn split_sale(&self, sell: i64) -> (i64, i64) {
        let mut from_farm = (sell + 1) / 2;
        let mut from_forest = sell - from_farm;
        if from_farm > self.farm {
            from_forest += from_farm - self.farm;
            from_farm = self.farm;
        }
        if from_forest > self.forest {
            from_farm += from_forest - self.forest;
            from_forest = self.forest;
        }
        (from_farm, from_forest)
    }

    pub fn farm_after(&self, o: &Orders) -> i64 {
        self.farm - self.split_sale(o.sell.max(0)).0
    }

    /// Money left after the orders, before the year's income.
    pub fn left(&self, o: &Orders) -> i64 {
        self.treasury + o.sell * self.price - o.food - o.plant * self.plant_cost - o.control
    }

    pub fn check(&self, o: &Orders) -> Option<String> {
        if o.sell < 0 || o.food < 0 || o.plant < 0 || o.control < 0 {
            return Some("Orders can't be negative.".into());
        }
        if o.sell > self.land() {
            return Some(format!("The nation holds only {} ha.", self.land()));
        }
        let farm = self.farm_after(o);
        if o.plant > farm {
            return Some(format!("Only {farm} ha of farmland would be left."));
        }
        if o.plant > self.citizens * HA_PER_FARMER {
            return Some(format!(
                "{} citizens can farm at most {} ha.",
                self.citizens,
                self.citizens * HA_PER_FARMER
            ));
        }
        if self.left(o) < 0 {
            return Some(format!(
                "That spends {} crowns more than the treasury holds.",
                -self.left(o)
            ));
        }
        None
    }

    /// How fouled the island is, 0 clean to 0.8 at worst, after the
    /// money spent on control (10 crowns clears one hectare's worth).
    pub fn pollution(industry: i64, control: i64) -> f32 {
        let units = (industry - control / 10).max(0) as f32;
        (units / 1000.0).min(0.8)
    }

    pub fn run_year(&mut self, o: &Orders, rng: &mut Rng) -> Year {
        let (from_farm, from_forest) = self.split_sale(o.sell);
        let mut y = Year {
            year: self.year,
            sold: o.sell,
            sale_money: o.sell * self.price,
            ..Year::default()
        };
        self.treasury = self.left(o);
        self.farm -= from_farm;
        self.forest -= from_forest;
        self.industry += o.sell;
        let poll = Nation::pollution(self.industry, o.control);
        y.pollution = poll;

        // food first
        let fed = o.food / FOOD;
        y.hungry = (self.citizens - fed).max(0);
        y.lost_hunger = y.hungry / 4;
        let hungry_leave = y.hungry / 2;
        // the air and the water
        y.lost_illness = (self.citizens as f32 * poll * 0.04).round() as i64;
        let poll_leave = (self.citizens as f32 * poll * 0.05).round() as i64;
        y.left = hungry_leave + poll_leave;
        let lost = y.lost_hunger + y.lost_illness;

        let before = self.citizens;
        if y.hungry == 0 && poll < 0.2 {
            y.born = (before as f32 * rng.between(0.02, 0.04)).round() as i64;
        }
        self.citizens = (before - lost - y.left + y.born).max(0);

        // the year's income
        let yield_ha = rng.range(18, 32) as f32 * (1.0 - poll * 0.5);
        y.harvest = (o.plant as f32 * yield_ha) as i64;
        y.tourism = (self.forest as f32 * rng.between(10.0, 16.0) * (1.0 - poll)) as i64;
        y.workers_came = from_farm + from_forest;
        y.workers_came /= 2;
        self.workers += y.workers_came;
        y.levies = self.industry * 15 + self.workers * 40;
        self.treasury += y.harvest + y.tourism + y.levies;

        y.citizens = self.citizens;
        y.treasury = self.treasury;
        y.forest = self.forest;
        self.history.push(y.clone());

        if lost * 100 > before * 8 {
            self.over = Some(End::NoConfidence);
        } else if y.hungry * 10 > before && self.treasury > before * FOOD * 2 {
            self.over = Some(End::Hoarding);
        } else if self.citizens < self.start_citizens * 3 / 5 {
            self.over = Some(End::Emptied);
        } else {
            self.year += 1;
            if self.year > YEARS {
                self.over = Some(End::Served(self.score(poll)));
            }
        }
        self.price = rng.range(90, 120);
        self.plant_cost = rng.range(10, 15);
        y
    }

    /// Out of 100: people kept, forest kept, clean air, money in hand.
    pub fn score(&self, poll: f32) -> u32 {
        let people = (self.citizens as f32 / self.start_citizens as f32).min(1.2) / 1.2 * 50.0;
        let forest = self.forest as f32 / 1000.0 * 20.0;
        let clean = (1.0 - poll / 0.8) * 20.0;
        let money = (self.treasury as f32 / 30_000.0).clamp(0.0, 1.0) * 10.0;
        (people + forest + clean + money).round() as u32
    }

    /// A careful premier: feed everyone, plant what can be worked, keep
    /// the factories clean, and sell land only to pay for all that.
    pub fn steward(&self) -> Orders {
        let mut o = Orders {
            sell: 0,
            food: self.citizens * FOOD,
            plant: 0,
            control: 0,
        };
        o.plant = self.farm.min(self.citizens * HA_PER_FARMER);
        o.control = self.industry * 7;
        let reserve = 12_000;
        let short = reserve - self.left(&o);
        if short > 0 {
            o.sell = ((short + self.price - 1) / self.price).min(self.land());
            o.control += o.sell * 7;
            o.plant = o.plant.min(self.farm_after(&o));
        }
        // still short: trim the cleanup, then the planting
        while self.left(&o) < 0 && o.control > 0 {
            o.control = (o.control - 500).max(0);
        }
        while self.left(&o) < 0 && o.plant > 0 {
            o.plant = (o.plant - 10).max(0);
        }
        while self.left(&o) < 0 && o.food > 0 {
            o.food = (o.food - FOOD).max(0);
        }
        o
    }
}

pub fn verdict_words(score: u32) -> (&'static str, &'static str) {
    if score >= 85 {
        (
            "Re-elected by a landslide",
            "The island is greener, fuller and richer than you found it. Crowds line the harbour road to cheer you back into office.",
        )
    } else if score >= 70 {
        (
            "Re-elected",
            "A steady term. The people trust you with another, and the newspapers are kind.",
        )
    } else if score >= 55 {
        (
            "Narrowly re-elected",
            "The count runs late into the night. You keep your seat by a handful of votes, and a long list of promises.",
        )
    } else {
        (
            "Defeated at the polls",
            "The voters thank you for your service and choose someone else to lead Pellora.",
        )
    }
}

/// "1 citizen", "3 citizens"
fn count(n: i64, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

const FIELDS: usize = 4;

pub struct King {
    ctx: Ctx,
    nation: Nation,
    fields: [NumField; FIELDS],
    focus: usize,
    fresh: bool,
    note: String,
    show_help: bool,
    record: Kv,
    quit: bool,
    demo_clock: f32,
    field_rows: Vec<Rect>,
}

impl King {
    fn new_term(&mut self) {
        self.nation = Nation::new(&mut self.ctx.rng);
        self.reset_fields();
        self.note.clear();
        log::info!("new term");
    }

    fn reset_fields(&mut self) {
        let n = &self.nation;
        self.fields = [
            NumField::new("Land to sell to industry (ha)", 0, 0, n.land(), 10),
            NumField::new(
                "Crowns for the people's food",
                (n.citizens * FOOD).min(n.treasury),
                0,
                9_999_999,
                FOOD * 10,
            ),
            NumField::new("Farmland to plant (ha)", 0, 0, 99_999, 10),
            NumField::new("Crowns against pollution", 0, 0, 9_999_999, 500),
        ];
        let control = (n.industry * 10)
            .min(n.treasury - self.fields[1].value())
            .max(0);
        self.fields[3].set(control);
        let plant = n
            .farm
            .min(n.citizens * HA_PER_FARMER)
            .min(((n.treasury - self.fields[1].value() - control) / n.plant_cost.max(1)).max(0));
        self.fields[2].set(plant);
        self.focus = 0;
        self.fresh = true;
        if self.ctx.demo && self.nation.over.is_none() {
            // the demo premier shows its orders a while before the year ends,
            // and is tempted by industry now and then
            let mut o = self.nation.steward();
            if self.ctx.rng.chance(0.35) {
                o.sell += self.ctx.rng.range(5, 15) * 10;
                o.sell = o.sell.min(self.nation.land());
                o.plant = o.plant.min(self.nation.farm_after(&o));
                o.control += o.sell * 7;
                while self.nation.left(&o) < 0 && o.control > 0 {
                    o.control = (o.control - 500).max(0);
                }
            }
            self.fields[0].set(o.sell);
            self.fields[1].set(o.food);
            self.fields[2].set(o.plant);
            self.fields[3].set(o.control);
        }
    }

    fn orders(&self) -> Orders {
        Orders {
            sell: self.fields[0].value(),
            food: self.fields[1].value(),
            plant: self.fields[2].value(),
            control: self.fields[3].value(),
        }
    }

    fn end_year(&mut self) {
        if self.nation.over.is_some() {
            return;
        }
        let o = self.orders();
        if let Some(why) = self.nation.check(&o) {
            self.note = why;
            self.ctx.audio.play(Sound::Nope);
            return;
        }
        let y = self.nation.run_year(&o, &mut self.ctx.rng);
        log::info!("year {} orders {:?} -> {:?}", y.year, o, y);
        match self.nation.over {
            Some(End::Served(s)) => {
                self.ctx
                    .audio
                    .play(if s >= 55 { Sound::Win } else { Sound::Lose });
                self.save_record(Some(s));
            }
            Some(_) => {
                self.ctx.audio.play(Sound::Lose);
                self.save_record(None);
            }
            None => self.ctx.audio.play(if y.hungry > 0 || y.lost_illness > 0 {
                Sound::Bad
            } else {
                Sound::Good
            }),
        }
        self.note.clear();
        self.reset_fields();
    }

    fn save_record(&mut self, score: Option<u32>) {
        if self.ctx.demo {
            return;
        }
        let terms = self.record.get::<u32>("terms").unwrap_or(0) + 1;
        self.record.set("terms", terms);
        if let Some(s) = score {
            let served = self.record.get::<u32>("served").unwrap_or(0) + 1;
            self.record.set("served", served);
            if s > self.record.get::<u32>("best").unwrap_or(0) {
                self.record.set("best", s);
            }
        }
        self.record.save(Self::ID, "record");
    }

    fn report(&self) -> Vec<(String, Color)> {
        let n = &self.nation;
        let mut out = Vec::new();
        let Some(y) = n.history.last() else {
            return out;
        };
        if y.year == 0 {
            out.push((
                format!("You are sworn in as premier of {ISLAND}, an island of farms, forest and fishing harbours."),
                INK,
            ));
            out.push((
                "Foreign companies want land for factories. They pay well, and they are not clean."
                    .into(),
                DIM,
            ));
        } else {
            if y.sold > 0 {
                out.push((
                    format!(
                        "{} ha sold to industry for {} crowns.",
                        y.sold, y.sale_money
                    ),
                    INK,
                ));
            }
            if y.hungry == 0 {
                out.push(("Every citizen had enough to eat.".into(), GREEN));
            } else {
                out.push((
                    format!("{} went hungry.", count(y.hungry, "citizen", "citizens")),
                    RED,
                ));
                if y.lost_hunger > 0 {
                    out.push((
                        format!("{} lost to hunger.", count(y.lost_hunger, "was", "were")),
                        RED,
                    ));
                }
            }
            if y.lost_illness > 0 {
                out.push((
                    format!(
                        "{} lost to illness from the bad air and water.",
                        count(y.lost_illness, "was", "were")
                    ),
                    RED,
                ));
            }
            if y.left > 0 {
                out.push((
                    format!(
                        "{} left the island for good.",
                        count(y.left, "citizen", "citizens")
                    ),
                    AMBER,
                ));
            }
            if y.born > 0 {
                out.push((
                    format!("{} born.", count(y.born, "baby was", "babies were")),
                    GREEN,
                ));
            }
            if y.workers_came > 0 {
                out.push((
                    format!(
                        "{} from abroad to the factories.",
                        count(y.workers_came, "worker came", "workers came")
                    ),
                    DIM,
                ));
            }
            out.push((
                format!(
                    "Harvest {}, tourists {}, levies {} crowns.",
                    y.harvest, y.tourism, y.levies
                ),
                INK,
            ));
            if y.pollution >= 0.05 {
                out.push((
                    format!("Pollution cost {:.0}% of tourism.", y.pollution * 100.0),
                    AMBER,
                ));
            }
        }
        out.push((String::new(), INK));
        out.push((
            format!("Citizens {}   Workers {}", n.citizens, n.workers),
            CYAN,
        ));
        out.push((format!("Treasury {} crowns", n.treasury), CYAN));
        if n.over.is_none() {
            out.push((
                format!(
                    "Industry offers {} a ha; planting costs {} a ha.",
                    n.price, n.plant_cost
                ),
                DIM,
            ));
        }
        out
    }

    /// The island as a little map: forest, farms and factories in their
    /// shares, a haze over it all as pollution rises.
    fn draw_island(&self, buf: &mut Buffer, r: Rect) {
        let n = &self.nation;
        let p = panel(buf, r, &format!("The island of {ISLAND}"), GREEN);
        if p.height < 4 || p.width < 12 {
            return;
        }
        let map = Rect {
            height: p.height.saturating_sub(2),
            ..p
        };
        let (cx, cy) = (
            map.x as f32 + map.width as f32 / 2.0,
            map.y as f32 + map.height as f32 / 2.0,
        );
        let (rx, ry) = (map.width as f32 / 2.0 - 1.0, map.height as f32 / 2.0 - 0.3);
        // island cells in a fixed order: east coast first (industry),
        // then the low middle (farms), then the hills (forest)
        let mut cells = Vec::new();
        for y in map.top()..map.bottom() {
            for x in map.left()..map.right() {
                let dx = (x as f32 + 0.5 - cx) / rx;
                let dy = (y as f32 + 0.5 - cy) / ry;
                let wobble = 0.12 * ((x as f32 * 0.7).sin() + (y as f32 * 1.3).cos());
                if dx * dx + dy * dy <= 1.0 + wobble {
                    cells.push((x, y, dx, dy));
                }
            }
        }
        let poll = n
            .history
            .last()
            .map(|y| y.pollution)
            .unwrap_or(0.0)
            .max(Nation::pollution(n.industry, 0) * 0.3);
        let total = (n.land() + n.industry).max(1) as f32;
        let ni = (cells.len() as f32 * n.industry as f32 / total).round() as usize;
        let nf = (cells.len() as f32 * n.farm as f32 / total).round() as usize;
        let mut order: Vec<usize> = (0..cells.len()).collect();
        order.sort_by(|&a, &b| {
            let ka = cells[a].2 - cells[a].3 * 0.3;
            let kb = cells[b].2 - cells[b].3 * 0.3;
            kb.partial_cmp(&ka).unwrap_or(std::cmp::Ordering::Equal)
        });
        let haze = Color::Rgb(120, 112, 96);
        for (rank, &i) in order.iter().enumerate() {
            let (x, y, _, _) = cells[i];
            let (sym, fg) = if rank < ni {
                (
                    if (x + y) % 3 == 0 { "▙" } else { "▪" },
                    Color::Rgb(170, 160, 150),
                )
            } else if rank < ni + nf {
                (if (x + 2 * y) % 4 == 0 { "\"" } else { "·" }, AMBER)
            } else {
                (if (x * 3 + y) % 5 == 0 { "♠" } else { "♣" }, GREEN)
            };
            let fg = mix(fg, haze, poll * 1.1);
            buf[(x, y)]
                .set_symbol(sym)
                .set_style(on(fg, Color::Rgb(18, 26, 22)));
        }
        let ly = p.bottom() - 1;
        let mut x = p.x;
        let wide = p.width >= 44;
        for (sym, c, word, v) in [
            ("♣", GREEN, "forest", n.forest),
            ("·", AMBER, "farms", n.farm),
            ("▪", Color::Rgb(170, 160, 150), "industry", n.industry),
        ] {
            let label = if wide {
                format!("{word} {v}")
            } else {
                format!("{v}")
            };
            x = put(buf, x, ly, sym, st(c));
            x = put(buf, x + 1, ly, &label, st(DIM)) + 2;
        }
    }

    fn draw_chart(&self, buf: &mut Buffer, r: Rect) {
        let p = panel(buf, r, "By year", AMBER);
        if p.height < 3 || p.width < 20 {
            return;
        }
        type Series<'a> = (&'a str, Color, Box<dyn Fn(&Year) -> i64>);
        let series: [Series; 3] = [
            ("people", CYAN, Box::new(|y: &Year| y.citizens)),
            ("crowns", AMBER, Box::new(|y: &Year| y.treasury.max(0))),
            ("forest", GREEN, Box::new(|y: &Year| y.forest)),
        ];
        const B: [&str; 9] = [" ", "▁", "▂", "▃", "▄", "▅", "▆", "▇", "█"];
        for (si, (name, colour, f)) in series.iter().enumerate() {
            let y = p.y + si as u16;
            if y >= p.bottom() {
                break;
            }
            put(buf, p.x, y, name, st(*colour));
            let max = self.nation.history.iter().map(f).max().unwrap_or(1).max(1) as f32;
            for (i, h) in self.nation.history.iter().enumerate() {
                let v = (f(h) as f32 / max * 8.0).round() as usize;
                put(buf, p.x + 8 + i as u16 * 2, y, B[v.min(8)], st(*colour));
            }
            let last = self.nation.history.last().map(&**f).unwrap_or(0);
            let label = last.to_string();
            put(
                buf,
                p.right().saturating_sub(label.len() as u16),
                y,
                &label,
                bold(st(*colour)),
            );
        }
    }
}

impl Game for King {
    const ID: &'static str = "king";
    const TITLE: &'static str = "King";
    const HELP: &'static str = "Eight years as premier of Pellora, a small island nation. Each year
decide how much land to sell to foreign industry, how many crowns go to
feed the people (80 a citizen), how much farmland to plant (each citizen
farms up to 2 ha) and how much to spend against pollution. Factories pay
well and bring workers and levies, but foul the air and water: harvests
shrink, tourists stay away, people fall ill and leave. Lose too many in a
year, hoard money while people go hungry, or empty the island, and the
assembly ends your term early. Serve all eight years and face the voters.
Tab or the arrows move between orders, Up/Down change a number, typing
replaces it, Enter ends the year. F feeds everyone, P plants all you can.";

    fn new(ctx: Ctx) -> Self {
        let mut ctx = ctx;
        let nation = Nation::new(&mut ctx.rng);
        let mut g = King {
            ctx,
            nation,
            fields: [
                NumField::new("", 0, 0, 0, 1),
                NumField::new("", 0, 0, 0, 1),
                NumField::new("", 0, 0, 0, 1),
                NumField::new("", 0, 0, 0, 1),
            ],
            focus: 0,
            fresh: true,
            note: String::new(),
            show_help: false,
            record: Kv::load("king", "record"),
            quit: false,
            demo_clock: 0.0,
            field_rows: Vec::new(),
        };
        g.reset_fields();
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
                self.quit = true;
                return;
            }
            Key::Char('m') => {
                self.ctx.audio.toggle();
                crate::shell::remember_mute(Self::ID, self.ctx.audio.muted);
                return;
            }
            Key::Char('?') => {
                self.show_help = !self.show_help;
                return;
            }
            _ => {}
        }
        if self.nation.over.is_some() {
            if matches!(k, Key::Enter | Key::Char('n')) {
                self.new_term();
                self.ctx.audio.play(Sound::Click);
            }
            if k == Key::Tab {
                self.show_help = !self.show_help;
            }
            return;
        }
        match k {
            Key::Tab | Key::Right => {
                self.focus = (self.focus + 1) % FIELDS;
                self.fresh = true;
                self.ctx.audio.play(Sound::Tick);
            }
            Key::BackTab | Key::Left => {
                self.focus = (self.focus + FIELDS - 1) % FIELDS;
                self.fresh = true;
                self.ctx.audio.play(Sound::Tick);
            }
            Key::Char('f') => {
                let need = self.nation.citizens * FOOD;
                self.fields[1].set(need);
            }
            Key::Char('p') => {
                let o = self.orders();
                let n = &self.nation;
                let afford = (n.left(&Orders { plant: 0, ..o }) / n.plant_cost.max(1)).max(0);
                self.fields[2].set(n.farm_after(&o).min(n.citizens * HA_PER_FARMER).min(afford));
            }
            Key::Enter => self.end_year(),
            other => {
                let mut fresh = self.fresh;
                if self.fields[self.focus].key(&other, &mut fresh) {
                    self.fresh = fresh;
                }
            }
        }
    }

    fn mouse(&mut self, m: Mouse) {
        if self.ctx.demo || self.nation.over.is_some() {
            if m.kind == MouseKind::Click && !self.ctx.demo {
                self.key(Key::Enter);
            }
            return;
        }
        match m.kind {
            MouseKind::Click => {
                if let Some(i) = self.field_rows.iter().position(|r| hit(r, m.x, m.y)) {
                    self.focus = i;
                    self.fresh = true;
                }
            }
            MouseKind::ScrollUp => {
                let mut f = true;
                self.fields[self.focus].key(&Key::Up, &mut f);
            }
            MouseKind::ScrollDown => {
                let mut f = true;
                self.fields[self.focus].key(&Key::Down, &mut f);
            }
            _ => {}
        }
    }

    fn tick(&mut self, dt: f32) {
        if !self.ctx.demo {
            return;
        }
        self.demo_clock += dt;
        let wait = if self.nation.over.is_some() { 5.0 } else { 2.8 };
        if self.demo_clock < wait {
            return;
        }
        self.demo_clock = 0.0;
        if self.nation.over.is_some() {
            self.new_term();
            return;
        }
        if self.nation.check(&self.orders()).is_some() {
            let o = self.nation.steward();
            self.fields[0].set(o.sell);
            self.fields[1].set(o.food);
            self.fields[2].set(o.plant);
            self.fields[3].set(o.control);
        }
        self.end_year();
    }

    fn quit(&self) -> bool {
        self.quit
    }

    fn snapshot(&self) -> String {
        let n = &self.nation;
        format!(
            "year {} citizens {} workers {} treasury {} forest {} farm {} industry {} price {} focus {} over {:?}",
            n.year, n.citizens, n.workers, n.treasury, n.forest, n.farm, n.industry, n.price, self.focus, n.over
        )
    }

    fn draw(&mut self, area: Rect, buf: &mut Buffer) {
        self.field_rows.clear();
        let n = &self.nation;
        let status = if n.over.is_some() {
            "the term is over".to_string()
        } else {
            format!("year {} of {YEARS}", n.year)
        };
        let keys: &[(&str, &str)] = if n.over.is_some() {
            &[
                ("Enter", "new term"),
                ("?", "rules"),
                ("M", "sound"),
                ("Esc", "quit"),
            ]
        } else {
            &[
                ("Tab", "next"),
                ("↑↓", "change"),
                ("0-9", "type"),
                ("F", "feed all"),
                ("P", "plant all"),
                ("Enter", "end year"),
                ("?", "rules"),
                ("M", "sound"),
                ("Esc", "quit"),
            ]
        };
        let inner = frame(buf, area, "King", &status, keys);
        if too_small(buf, inner, 78, 22) {
            return;
        }
        let left_w = (inner.width / 2).clamp(40, 50);
        let left = Rect {
            width: left_w,
            ..inner
        };
        let right = Rect {
            x: inner.x + left_w,
            width: inner.width - left_w,
            ..inner
        };

        // the right side: the island, and the years in a chart
        let chart_h = 5.min(right.height / 3);
        self.draw_island(
            buf,
            Rect {
                height: right.height - chart_h,
                ..right
            },
        );
        self.draw_chart(
            buf,
            Rect {
                y: right.bottom() - chart_h,
                height: chart_h,
                ..right
            },
        );

        let orders_h = if n.over.is_some() { 0 } else { 12 };
        let report = Rect {
            height: left.height - orders_h,
            ..left
        };
        if self.show_help {
            let p = panel(buf, report, "How Pellora works", CYAN);
            para(buf, p, "Each citizen needs 80 crowns of food a year and can farm 2 ha. A planted hectare costs 10 to 15 crowns and brings back 18 to 32 at harvest, less in a fouled year. Forest brings tourists. Land sold to industry pays once, then the factories pay levies each year and bring foreign workers, who pay theirs too; but every factory hectare fouls the island unless 10 crowns a year clean up after it. Lose more than 8% of the citizens in a year, hoard while a tenth go hungry, or let the island fall below three fifths of its people, and your term ends early.", st(INK));
        } else {
            let title = match n.history.last() {
                Some(y) if y.year == 0 => "The island as you find it".to_string(),
                Some(y) => format!("Report for year {}", y.year),
                None => String::new(),
            };
            let p = panel(buf, report, &title, AMBER);
            let mut y = p.y;
            let mut lines = self.report();
            if let Some(end) = n.over {
                lines.push((String::new(), INK));
                let (head, body) = match end {
                    End::NoConfidence => ("A vote of no confidence".to_string(), "So many lost in one year was more than the assembly would bear. It votes you out of office, and a caretaker takes your desk.".to_string()),
                    End::Hoarding => ("Removed by the assembly".to_string(), "People went hungry while the treasury sat full. The assembly calls a vote, and you are out by nightfall.".to_string()),
                    End::Emptied => ("An empty island".to_string(), "So many have left that the harbours are quiet and the schools half closed. You resign before the assembly can ask you to.".to_string()),
                    End::Served(s) => {
                        let (h, b) = verdict_words(s);
                        (format!("{h} (score {s} of 100)"), b.to_string())
                    }
                };
                let good = matches!(end, End::Served(s) if s >= 55);
                lines.push((head, if good { GREEN } else { RED }));
                lines.push((body, INK));
                lines.push((
                    format!(
                        "Terms served in full: {}   Best score: {}",
                        self.record.get::<u32>("served").unwrap_or(0),
                        self.record
                            .get::<u32>("best")
                            .map_or("-".into(), |b| b.to_string())
                    ),
                    DIM,
                ));
            }
            for (line, colour) in lines {
                if y >= p.bottom() {
                    break;
                }
                if line.is_empty() {
                    y += 1;
                    continue;
                }
                y += para(
                    buf,
                    Rect {
                        y,
                        height: p.bottom() - y,
                        ..p
                    },
                    &line,
                    st(colour),
                );
            }
        }

        if n.over.is_none() {
            let orders = Rect {
                y: left.bottom() - orders_h,
                height: orders_h,
                ..left
            };
            let p = panel(buf, orders, "Your decisions", AMBER);
            for (i, f) in self.fields.iter().enumerate() {
                let y = p.y + i as u16 * 2;
                if y + 1 < p.bottom() {
                    put(
                        buf,
                        p.x + 2,
                        y,
                        f.label,
                        st(if i == self.focus { INK } else { DIM }),
                    );
                    f.draw(buf, p.x + 2, y + 1, 0, 12, i == self.focus);
                    self.field_rows.push(Rect::new(p.x, y, p.width, 2));
                }
            }
            let o = self.orders();
            let msg = match self.nation.check(&o) {
                Some(why) => (why, RED),
                None if !self.note.is_empty() => (self.note.clone(), RED),
                None => {
                    let left = self.nation.left(&o);
                    let hungry = (self.nation.citizens - o.food / FOOD).max(0);
                    let poll = Nation::pollution(self.nation.industry + o.sell, o.control);
                    if hungry > 0 {
                        (format!("{left} kept. {hungry} would go hungry!"), AMBER)
                    } else if poll >= 0.05 {
                        (
                            format!("{left} kept. Pollution would be {:.0}%.", poll * 100.0),
                            AMBER,
                        )
                    } else {
                        (format!("{left} crowns kept in the treasury."), GREEN)
                    }
                }
            };
            put_max(buf, p.x, p.bottom() - 1, &msg.0, st(msg.1), p.width);
            // the numbers sit right of their boxes: what each order means
            let hx = p.x + 16;
            let w = p.right().saturating_sub(hx);
            let hints = [
                format!("of {} ha, at {}", self.nation.land(), self.nation.price),
                format!("{} feeds all", self.nation.citizens * FOOD),
                format!("at {} a ha", self.nation.plant_cost),
                format!(
                    "{} cleans it all",
                    (self.nation.industry + self.fields[0].value()) * 10
                ),
            ];
            for (i, h) in hints.iter().enumerate() {
                let y = p.y + i as u16 * 2 + 1;
                if y < p.bottom() - 1 {
                    put_max(buf, hx, y, h, st(DIM), w);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orders_are_checked() {
        let mut rng = Rng::new(7);
        let n = Nation::new(&mut rng);
        let ok = Orders {
            sell: 0,
            food: n.citizens * FOOD,
            plant: 900,
            control: 0,
        };
        assert!(n.check(&ok).is_none());
        assert!(n.check(&Orders { plant: 1001, ..ok }).is_some());
        assert!(n
            .check(&Orders {
                food: 9_000_000,
                ..ok
            })
            .is_some());
        assert!(n.check(&Orders { sell: 2001, ..ok }).is_some());
        // selling land takes farmland with it
        assert!(n
            .check(&Orders {
                sell: 400,
                plant: 900,
                ..ok
            })
            .is_some());
    }

    #[test]
    fn pollution_rises_with_industry_and_falls_with_control() {
        assert_eq!(Nation::pollution(0, 0), 0.0);
        assert!(Nation::pollution(300, 0) > Nation::pollution(300, 2000));
        assert_eq!(Nation::pollution(300, 3000), 0.0);
        assert!(Nation::pollution(5000, 0) <= 0.8);
    }

    #[test]
    fn letting_people_go_hungry_ends_the_term() {
        let mut rng = Rng::new(3);
        let mut n = Nation::new(&mut rng);
        n.run_year(&Orders::default(), &mut rng);
        assert!(matches!(n.over, Some(End::NoConfidence)));
    }

    #[test]
    fn a_careful_premier_serves_the_full_term() {
        for seed in 1..40 {
            let mut rng = Rng::new(seed);
            let mut n = Nation::new(&mut rng);
            while n.over.is_none() {
                let o = n.steward();
                assert!(n.check(&o).is_none(), "seed {seed} year {} {o:?}", n.year);
                n.run_year(&o, &mut rng);
            }
            assert!(
                matches!(n.over, Some(End::Served(_))),
                "seed {seed}: {:?}",
                n.over
            );
        }
    }
}

#[cfg(test)]
mod trace {
    use super::*;
    #[test]
    #[ignore]
    fn print_steward_scores() {
        for seed in 1..15 {
            let mut rng = Rng::new(seed);
            let mut n = Nation::new(&mut rng);
            while n.over.is_none() {
                let o = n.steward();
                n.run_year(&o, &mut rng);
            }
            println!(
                "seed {seed}: {:?} citizens {} forest {} industry {} treasury {}",
                n.over, n.citizens, n.forest, n.industry, n.treasury
            );
        }
    }
}
