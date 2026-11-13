//! Hammurabi: ten years ruling a city of ancient Sumer. Rules in
//! asciiverse/specs/hammurabi.md.

use crate::audio::Sound;
use crate::rng::Rng;
use crate::shell::{Ctx, Game, Key, Mouse, MouseKind};
use crate::store::Kv;
use crate::ui::*;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

pub const YEARS: u32 = 10;
pub const FOOD_PER_PERSON: i64 = 20;
pub const ACRES_PER_WORKER: i64 = 10;
/// seed grain for each acre planted, in halves of a bushel
pub const SEED_PER_TWO_ACRES: i64 = 1;

/// What happened in one year, as the chronicle reports it.
#[derive(Clone, Debug, Default)]
pub struct Year {
    pub year: u32,
    pub starved: i64,
    pub arrived: i64,
    pub plague: bool,
    pub yield_per_acre: i64,
    pub harvest: i64,
    pub rats: i64,
    pub population: i64,
    pub acres: i64,
    pub grain: i64,
    pub price: i64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Verdict {
    /// too many starved in a single year
    Deposed,
    Tyrant,
    Harsh,
    Fair,
    Wise,
}

/// The city and its books.
#[derive(Clone, Debug)]
pub struct City {
    pub year: u32,
    pub population: i64,
    pub acres: i64,
    pub grain: i64,
    pub price: i64,
    pub history: Vec<Year>,
    pub total_starved: i64,
    pub starved_pct_sum: f64,
    pub over: Option<Verdict>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Orders {
    /// acres bought (positive) or sold (negative)
    pub land: i64,
    pub feed: i64,
    pub plant: i64,
}

impl City {
    /// The city as the new ruler finds it, after a first, modest year.
    pub fn new(rng: &mut Rng) -> City {
        let y = Year {
            year: 0,
            starved: 0,
            arrived: 5,
            plague: false,
            yield_per_acre: 3,
            harvest: 3000,
            rats: 200,
            population: 100,
            acres: 1000,
            grain: 2800,
            price: rng.range(17, 26),
        };
        City {
            year: 1,
            population: 100,
            acres: 1000,
            grain: 2800,
            price: y.price,
            history: vec![y],
            total_starved: 0,
            starved_pct_sum: 0.0,
            over: None,
        }
    }

    /// Why these orders can't be carried out, if they can't.
    pub fn check(&self, o: &Orders) -> Option<String> {
        let acres = self.acres + o.land;
        if acres < 0 {
            return Some(format!("You hold only {} acres to sell.", self.acres));
        }
        let mut grain = self.grain - o.land * self.price;
        if grain < 0 {
            return Some(format!(
                "That land costs {} bushels; the stores hold {}.",
                o.land * self.price,
                self.grain
            ));
        }
        if o.feed < 0 || o.plant < 0 {
            return Some("Orders can't be negative.".into());
        }
        grain -= o.feed;
        if grain < 0 {
            return Some("There isn't that much grain to hand out.".into());
        }
        if o.plant > acres {
            return Some(format!("You'll own only {acres} acres."));
        }
        if o.plant > self.population * ACRES_PER_WORKER {
            return Some(format!(
                "{} people can work at most {} acres.",
                self.population,
                self.population * ACRES_PER_WORKER
            ));
        }
        if o.plant * SEED_PER_TWO_ACRES / 2 > grain {
            return Some("Not enough grain left over to seed that many acres.".into());
        }
        None
    }

    /// Grain left in store after these orders, before the harvest.
    pub fn left(&self, o: &Orders) -> i64 {
        self.grain - o.land * self.price - o.feed - (o.plant * SEED_PER_TWO_ACRES + 1) / 2
    }

    /// Carry out a year's orders (already checked) and see what the year brings.
    pub fn run_year(&mut self, o: &Orders, rng: &mut Rng) -> Year {
        self.acres += o.land;
        self.grain = self.left(o);
        let fed = o.feed / FOOD_PER_PERSON;
        let starved = (self.population - fed).max(0);
        let starved_pct = starved as f64 / self.population as f64 * 100.0;
        self.starved_pct_sum += starved_pct;
        self.total_starved += starved;
        let mut y = Year {
            year: self.year,
            starved,
            ..Year::default()
        };
        if starved_pct > 45.0 {
            self.population -= starved;
            y.population = self.population;
            y.acres = self.acres;
            y.grain = self.grain;
            self.history.push(y.clone());
            self.over = Some(Verdict::Deposed);
            return y;
        }
        self.population -= starved;
        y.yield_per_acre = rng.range(1, 5);
        y.harvest = y.yield_per_acre * o.plant;
        self.grain += y.harvest;
        if rng.chance(0.4) {
            y.rats = (self.grain as f32 * rng.between(0.05, 0.25)) as i64;
            self.grain -= y.rats;
        }
        // newcomers follow good harvests, wide fields and full stores
        if starved == 0 {
            let pull =
                (20 * self.acres + self.grain) as f64 / self.population.max(1) as f64 / 100.0;
            y.arrived =
                ((y.yield_per_acre as f64 * pull) / 3.0 + 1.0 + rng.range(0, 3) as f64) as i64;
        }
        self.population += y.arrived;
        if rng.chance(0.15) {
            y.plague = true;
            self.population /= 2;
        }
        self.price = rng.range(17, 26);
        y.population = self.population;
        y.acres = self.acres;
        y.grain = self.grain;
        y.price = self.price;
        self.history.push(y.clone());
        self.year += 1;
        if self.year > YEARS {
            self.over = Some(self.verdict());
        }
        y
    }

    pub fn average_starved_pct(&self) -> f64 {
        let years = self.history.iter().filter(|y| y.year > 0).count().max(1);
        self.starved_pct_sum / years as f64
    }

    pub fn acres_per_person(&self) -> f64 {
        self.acres as f64 / self.population.max(1) as f64
    }

    pub fn verdict(&self) -> Verdict {
        let p = self.average_starved_pct();
        let l = self.acres_per_person();
        if p > 33.0 || l < 7.0 {
            Verdict::Tyrant
        } else if p > 10.0 || l < 9.0 {
            Verdict::Harsh
        } else if p > 3.0 || l < 10.0 {
            Verdict::Fair
        } else {
            Verdict::Wise
        }
    }

    /// Orders a careful steward would give: seed the fields first (next
    /// year's food), feed what remains, never let more than a third go
    /// hungry, buy land only when it is cheap and the stores are full.
    pub fn steward(&self) -> Orders {
        let mut o = Orders::default();
        let workable = self.population * ACRES_PER_WORKER;
        let need = self.population * FOOD_PER_PERSON;
        if self.price <= 19 && self.acres < workable && self.grain > need + workable {
            let spare = self.grain - need - workable / 2;
            o.land = (spare / self.price / 2).clamp(0, workable - self.acres);
        } else if self.price >= 24 && self.acres > workable + 30 {
            o.land = -((self.acres - workable) / 2);
        }
        // hungry year: sell land for bread rather than lose the city
        let floor = need * 2 / 3;
        if self.grain - o.land * self.price < floor + 200 {
            let short = floor + 200 - (self.grain - o.land * self.price);
            o.land -= ((short + self.price - 1) / self.price).min(self.acres + o.land);
        }
        let grain = self.grain - o.land * self.price;
        let mut plant = (self.acres + o.land).min(workable).min(grain * 2).max(0);
        // keep at least two thirds of the people fed, even at the cost of seed
        if grain - plant / 2 < floor {
            plant = ((grain - floor) * 2).clamp(0, plant);
        }
        o.plant = plant;
        o.feed = need.min(grain - (plant + 1) / 2).max(0);
        o
    }
}

const FIELDS: usize = 3;

pub struct Hammurabi {
    ctx: Ctx,
    city: City,
    fields: [NumField; FIELDS],
    focus: usize,
    fresh: bool,
    note: String,
    best: Kv,
    quit: bool,
    demo_clock: f32,
    field_rows: Vec<Rect>,
}

impl Hammurabi {
    fn new_term(&mut self) {
        self.city = City::new(&mut self.ctx.rng);
        self.reset_fields();
        self.note = String::new();
        log::info!("new term");
    }

    fn reset_fields(&mut self) {
        let c = &self.city;
        self.fields = [
            NumField::new("Land to buy (+) or sell (-)", 0, -c.acres, 99_999, 10),
            NumField::new(
                "Grain to feed the people",
                (c.population * FOOD_PER_PERSON).min(c.grain),
                0,
                999_999,
                100,
            ),
            NumField::new("Acres to plant", 0, 0, 99_999, 10),
        ];
        let plant = c
            .acres
            .min(c.population * ACRES_PER_WORKER)
            .min((c.grain - self.fields[1].value()) * 2)
            .max(0);
        self.fields[2].set(plant);
        self.focus = 0;
        self.fresh = true;
    }

    fn orders(&self) -> Orders {
        Orders {
            land: self.fields[0].value(),
            feed: self.fields[1].value(),
            plant: self.fields[2].value(),
        }
    }

    fn end_year(&mut self) {
        if self.city.over.is_some() {
            return;
        }
        let o = self.orders();
        if let Some(why) = self.city.check(&o) {
            self.note = why;
            self.ctx.audio.play(Sound::Nope);
            return;
        }
        let y = self.city.run_year(&o, &mut self.ctx.rng);
        log::info!("year {} orders {:?} -> {:?}", y.year, o, y);
        match self.city.over {
            Some(Verdict::Deposed) => self.ctx.audio.play(Sound::Lose),
            Some(v) => {
                self.ctx
                    .audio
                    .play(if matches!(v, Verdict::Wise | Verdict::Fair) {
                        Sound::Win
                    } else {
                        Sound::Lose
                    });
                self.save_best();
            }
            None => self.ctx.audio.play(if y.plague || y.starved > 0 {
                Sound::Bad
            } else {
                Sound::Good
            }),
        }
        self.note = String::new();
        self.reset_fields();
    }

    fn save_best(&mut self) {
        if self.ctx.demo {
            return;
        }
        let terms = self.best.get::<u32>("terms").unwrap_or(0) + 1;
        self.best.set("terms", terms);
        let pop = self.city.population;
        if pop > self.best.get::<i64>("largest").unwrap_or(0) {
            self.best.set("largest", pop);
        }
        if self.city.verdict() == Verdict::Wise {
            let w = self.best.get::<u32>("wise").unwrap_or(0) + 1;
            self.best.set("wise", w);
        }
        self.best.save(Self::ID, "record");
    }

    fn chronicle(&self) -> Vec<(String, ratatui::style::Color)> {
        let c = &self.city;
        let mut out = Vec::new();
        let Some(y) = c.history.last() else {
            return out;
        };
        if y.year == 0 {
            out.push(("You take the seal of a city of 100 souls.".into(), INK));
            out.push((
                "Last year's harvest was 3 bushels an acre, and rats spoiled 200 bushels of it."
                    .into(),
                DIM,
            ));
        } else {
            if y.starved > 0 {
                out.push((format!("{} people starved.", y.starved), RED));
            } else {
                out.push(("No one went hungry.".into(), GREEN));
            }
            if y.arrived > 0 {
                out.push((format!("{} newcomers settled in the city.", y.arrived), INK));
            }
            if y.plague {
                out.push(("Plague! Half the city died of it.".into(), RED));
            }
            if c.over != Some(Verdict::Deposed) {
                out.push((
                    format!("The fields gave {} bushels an acre:", y.yield_per_acre),
                    INK,
                ));
                out.push((format!("{} bushels in all.", y.harvest), INK));
                if y.rats > 0 {
                    out.push((
                        format!("Rats got into the stores: {} bushels lost.", y.rats),
                        AMBER,
                    ));
                }
            }
        }
        out.push((String::new(), INK));
        out.push((format!("People      {}", c.population), INK));
        out.push((format!("Land        {} acres", c.acres), INK));
        out.push((format!("Grain       {} bushels", c.grain), INK));
        if c.over.is_none() {
            out.push((format!("Land sells at {} bushels an acre.", c.price), CYAN));
        }
        out
    }

    fn draw_chart(&self, buf: &mut Buffer, r: Rect) {
        let p = panel(buf, r, "The city by year", AMBER);
        if p.height < 6 || p.width < 20 {
            return;
        }
        type Series<'a> = (&'a str, ratatui::style::Color, Box<dyn Fn(&Year) -> i64>);
        let series: [Series; 3] = [
            ("people", CYAN, Box::new(|y: &Year| y.population)),
            ("grain", AMBER, Box::new(|y: &Year| y.grain)),
            ("acres", GREEN, Box::new(|y: &Year| y.acres)),
        ];
        let rows_each = (p.height / 3).max(2);
        let col_w = ((p.width.saturating_sub(8)) / (YEARS as u16 + 1)).clamp(1, 4);
        for (si, (name, colour, f)) in series.iter().enumerate() {
            let top = p.y + si as u16 * rows_each;
            let h = rows_each - 1;
            put(buf, p.x, top, name, st(*colour));
            let max = self.city.history.iter().map(f).max().unwrap_or(1).max(1) as f32;
            for (i, y) in self.city.history.iter().enumerate() {
                let v = f(y) as f32 / max;
                let x = p.x + 7 + i as u16 * col_w;
                let eighths = (v * h as f32 * 8.0).round() as u16;
                for row in 0..h {
                    let filled = eighths.saturating_sub(row * 8).min(8);
                    const B: [&str; 9] = [" ", "▁", "▂", "▃", "▄", "▅", "▆", "▇", "█"];
                    let cy = top + h - row;
                    for dx in 0..col_w.saturating_sub(1).max(1) {
                        if x + dx < p.right() && cy < p.bottom() {
                            buf[(x + dx, cy)]
                                .set_symbol(B[filled as usize])
                                .set_style(st(*colour));
                        }
                    }
                }
            }
            let last = self.city.history.last().map(&**f).unwrap_or(0);
            let label = last.to_string();
            put(
                buf,
                p.right().saturating_sub(label.len() as u16),
                top,
                &label,
                bold(st(*colour)),
            );
        }
    }
}

impl Game for Hammurabi {
    const ID: &'static str = "hammurabi";
    const TITLE: &'static str = "Hammurabi";
    const HELP: &'static str = "Rule a city of ancient Sumer for ten years. Each year, trade land,
hand out grain and sow the fields: every person eats 20 bushels a year,
can work 10 acres, and every 2 acres take a bushel of seed. Harvests,
rats, newcomers and plague do the rest. Let too many starve in one year
and you are thrown out; last ten years and the people judge your reign.
Tab or the arrows move between orders, Up/Down change a number, typing
replaces it, Enter ends the year. F feeds everyone, P plants all you can.";

    fn new(ctx: Ctx) -> Self {
        let mut ctx = ctx;
        let city = City::new(&mut ctx.rng);
        let mut g = Hammurabi {
            ctx,
            city,
            fields: [
                NumField::new("", 0, 0, 0, 1),
                NumField::new("", 0, 0, 0, 1),
                NumField::new("", 0, 0, 0, 1),
            ],
            focus: 0,
            fresh: true,
            note: String::new(),
            best: Kv::load("hammurabi", "record"),
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
        if self.city.over.is_some() {
            match k {
                Key::Enter | Key::Char('n') => {
                    self.new_term();
                    self.ctx.audio.play(Sound::Click);
                }
                Key::Esc | Key::Char('q') => self.quit = true,
                _ => {}
            }
            return;
        }
        match k {
            Key::Esc | Key::Char('q') => self.quit = true,
            Key::Char('m') => {
                self.ctx.audio.toggle();
                crate::shell::remember_mute(Self::ID, self.ctx.audio.muted);
            }
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
                let need = self.city.population * FOOD_PER_PERSON;
                self.fields[1].set(need);
            }
            Key::Char('p') => {
                let o = self.orders();
                let acres = self.city.acres + o.land;
                let seedable = (self.city.grain - o.land * self.city.price - o.feed) * 2;
                self.fields[2].set(
                    acres
                        .min(self.city.population * ACRES_PER_WORKER)
                        .min(seedable)
                        .max(0),
                );
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
        if self.demo_clock < 2.6 {
            return;
        }
        self.demo_clock = 0.0;
        if self.city.over.is_some() {
            self.new_term();
            return;
        }
        let o = self.city.steward();
        self.fields[0].set(o.land);
        self.fields[1].set(o.feed);
        self.fields[2].set(o.plant);
        self.end_year();
    }

    fn quit(&self) -> bool {
        self.quit
    }

    fn snapshot(&self) -> String {
        let c = &self.city;
        format!(
            "year {} pop {} acres {} grain {} price {} focus {} over {:?}",
            c.year, c.population, c.acres, c.grain, c.price, self.focus, c.over
        )
    }

    fn draw(&mut self, area: Rect, buf: &mut Buffer) {
        self.field_rows.clear();
        let c = &self.city;
        let status = if c.over.is_some() {
            "the reign is over".to_string()
        } else {
            format!("year {} of {YEARS}", c.year)
        };
        let keys: &[(&str, &str)] = if c.over.is_some() {
            &[("Enter", "new reign"), ("Esc", "quit")]
        } else {
            &[
                ("Tab", "next"),
                ("↑↓", "change"),
                ("0-9", "type"),
                ("F", "feed all"),
                ("P", "plant all"),
                ("Enter", "end year"),
                ("Esc", "quit"),
            ]
        };
        let inner = frame(buf, area, "Hammurabi", &status, keys);
        if too_small(buf, inner, 76, 22) {
            return;
        }
        let left_w = 44.min(inner.width / 2);
        let left = Rect {
            width: left_w,
            ..inner
        };
        let right = Rect {
            x: inner.x + left_w,
            width: inner.width - left_w,
            ..inner
        };

        let title = match c.history.last() {
            Some(y) if y.year == 0 => "The city as you find it".to_string(),
            Some(y) => format!("Report for year {}", y.year),
            None => String::new(),
        };
        let report_h = if c.over.is_some() {
            left.height
        } else {
            left.height.saturating_sub(9)
        };
        let p = panel(
            buf,
            Rect {
                height: report_h,
                ..left
            },
            &title,
            AMBER,
        );
        let mut y = p.y;
        for (line, colour) in self.chronicle() {
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

        if c.over.is_none() {
            let orders = Rect {
                y: left.y + report_h,
                height: left.height - report_h,
                ..left
            };
            let p = panel(buf, orders, "Your orders", AMBER);
            for (i, f) in self.fields.iter().enumerate() {
                let y = p.y + i as u16 * 2;
                if y < p.bottom() {
                    put(
                        buf,
                        p.x + 2,
                        y,
                        f.label,
                        st(if i == self.focus { INK } else { DIM }),
                    );
                    f.draw(
                        buf,
                        p.x + 2,
                        y + 1,
                        0,
                        p.width.saturating_sub(4).min(14),
                        i == self.focus,
                    );
                    self.field_rows.push(Rect::new(p.x, y, p.width, 2));
                }
            }
            let o = self.orders();
            let msg = match self.city.check(&o) {
                Some(why) => (why, RED),
                None if !self.note.is_empty() => (self.note.clone(), RED),
                None => {
                    let left = self.city.left(&o);
                    let hungry = (self.city.population - o.feed / FOOD_PER_PERSON).max(0);
                    if hungry > 0 {
                        (
                            format!("{left} bushels kept. {hungry} would go hungry!"),
                            AMBER,
                        )
                    } else {
                        (format!("{left} bushels kept back in store."), GREEN)
                    }
                }
            };
            let ny = p.bottom().saturating_sub(1);
            put_max(buf, p.x, ny, &msg.0, st(msg.1), p.width);
        }

        if let Some(v) = c.over {
            let chart_h = right.height.saturating_sub(12);
            self.draw_chart(
                buf,
                Rect {
                    height: chart_h,
                    ..right
                },
            );
            let r = Rect {
                y: right.y + chart_h,
                height: right.height - chart_h,
                ..right
            };
            let p = panel(
                buf,
                r,
                "The people's judgement",
                if matches!(v, Verdict::Wise | Verdict::Fair) {
                    GREEN
                } else {
                    RED
                },
            );
            let text = match v {
                Verdict::Deposed => format!("You let {} people starve in a single year. The city rose, and you were dragged from the palace and sent into exile.", c.history.last().map_or(0, |y| y.starved)),
                Verdict::Tyrant => "Your reign is remembered with dread: the people curse your name and no one mourns its end.".into(),
                Verdict::Harsh => "You ruled with a heavy hand. The city survived you, and the people are glad to see you go.".into(),
                Verdict::Fair => "A fair reign, if not a great one. Some grumble, but most would have you back.".into(),
                Verdict::Wise => "A golden age! The people sing your name; no ruler of Sumer has done better.".into(),
            };
            let used = para(buf, p, &text, st(INK));
            let stats = [
                format!("Starved on average  {:.1}% a year", c.average_starved_pct()),
                format!("Starved in all      {}", c.total_starved),
                format!("Land per person     {:.1} acres", c.acres_per_person()),
                format!(
                    "Reigns served       {}",
                    self.best.get::<u32>("terms").unwrap_or(0)
                ),
            ];
            for (i, s) in stats.iter().enumerate() {
                let y = p.y + used + 1 + i as u16;
                if y < p.bottom() {
                    put(buf, p.x, y, s, st(DIM));
                }
            }
        } else {
            self.draw_chart(buf, right);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orders_are_checked() {
        let mut rng = Rng::new(7);
        let c = City::new(&mut rng);
        assert!(c
            .check(&Orders {
                land: 0,
                feed: 2000,
                plant: 1000
            })
            .is_none());
        assert!(c
            .check(&Orders {
                land: -2000,
                feed: 0,
                plant: 0
            })
            .is_some());
        assert!(c
            .check(&Orders {
                land: 0,
                feed: 2000,
                plant: 1001
            })
            .is_some());
        assert!(c
            .check(&Orders {
                land: 0,
                feed: 9000,
                plant: 0
            })
            .is_some());
    }

    #[test]
    fn starving_half_the_city_ends_the_reign() {
        let mut rng = Rng::new(3);
        let mut c = City::new(&mut rng);
        c.run_year(
            &Orders {
                land: 0,
                feed: 0,
                plant: 0,
            },
            &mut rng,
        );
        assert_eq!(c.over, Some(Verdict::Deposed));
    }

    #[test]
    fn a_careful_steward_lasts_ten_years() {
        for seed in 1..40 {
            let mut rng = Rng::new(seed);
            let mut c = City::new(&mut rng);
            while c.over.is_none() {
                let o = c.steward();
                assert!(
                    c.check(&o).is_none(),
                    "seed {seed} year {} orders {o:?}",
                    c.year
                );
                c.run_year(&o, &mut rng);
            }
            assert_ne!(c.over, Some(Verdict::Deposed), "seed {seed}");
        }
    }
}

#[cfg(test)]
mod trace {
    use super::*;
    #[test]
    #[ignore]
    fn print_steward() {
        let mut rng = Rng::new(5);
        let mut c = City::new(&mut rng);
        while c.over.is_none() {
            let o = c.steward();
            let before = (c.population, c.acres, c.grain, c.price);
            let y = c.run_year(&o, &mut rng);
            println!(
                "{before:?} {o:?} -> yield {} harvest {} rats {} arrived {} plague {} grain {}",
                y.yield_per_acre, y.harvest, y.rats, y.arrived, y.plague, y.grain
            );
        }
    }
}
