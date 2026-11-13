//! Civil War: fourteen battles of the American Civil War, refought. You
//! supply and command the Confederate army; the computer, or a second
//! player, commands the Union. Each battle is measured against what really
//! happened. Rules in asciiverse/specs/civil-war.md.

use crate::audio::Sound;
use crate::rng::Rng;
use crate::shell::{Ctx, Game, Key, Mouse, MouseKind};
use crate::ui::*;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Role {
    /// the Confederates attacked
    Attack,
    /// the Confederates defended
    Defend,
    /// both sides attacked
    Both,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Outcome {
    South,
    North,
    Drawn,
}

/// A battle as history records it, rounded: men engaged and casualties
/// (killed, wounded, missing and captured) on each side.
pub struct Battle {
    pub name: &'static str,
    pub date: &'static str,
    pub about: &'static str,
    pub south_men: u32,
    pub north_men: u32,
    pub south_lost: u32,
    pub north_lost: u32,
    pub role: Role,
    pub result: Outcome,
}

pub const BATTLES: &[Battle] = &[
    Battle { name: "First Bull Run", date: "July 21, 1861", about: "Beauregard and Johnston meet McDowell's advance on Manassas Junction. Jackson's brigade holds Henry House Hill, and the Union army breaks and streams back to Washington.", south_men: 18_000, north_men: 18_500, south_lost: 1_980, north_lost: 2_900, role: Role::Defend, result: Outcome::South },
    Battle { name: "Shiloh", date: "April 6-7, 1862", about: "A. S. Johnston surprises Grant at Pittsburg Landing and drives him to the river, but Johnston is killed, Buell's army arrives in the night, and the second day goes to the Union.", south_men: 44_700, north_men: 65_000, south_lost: 10_700, north_lost: 13_050, role: Role::Attack, result: Outcome::North },
    Battle { name: "Seven Days", date: "June 25 - July 1, 1862", about: "Lee attacks McClellan again and again outside Richmond. He loses more men than he inflicts, but McClellan retreats to the James and the capital is saved.", south_men: 92_000, north_men: 104_000, south_lost: 20_200, north_lost: 15_850, role: Role::Attack, result: Outcome::South },
    Battle { name: "Second Bull Run", date: "August 28-30, 1862", about: "Pope hammers at Jackson along an unfinished railroad cut while Longstreet arrives unseen on his flank, then strikes. The Union army is routed from the field.", south_men: 50_000, north_men: 70_000, south_lost: 8_350, north_lost: 14_460, role: Role::Both, result: Outcome::South },
    Battle { name: "Antietam", date: "September 17, 1862", about: "Lee's invasion of Maryland ends at Sharpsburg, the bloodiest single day of the war. His thin line holds against McClellan's piecemeal attacks, but he withdraws to Virginia.", south_men: 38_000, north_men: 75_000, south_lost: 10_320, north_lost: 12_400, role: Role::Defend, result: Outcome::North },
    Battle { name: "Fredericksburg", date: "December 13, 1862", about: "Burnside throws wave after wave at the stone wall below Marye's Heights. Lee's men, behind it, hardly need to move.", south_men: 72_500, north_men: 114_000, south_lost: 5_380, north_lost: 12_650, role: Role::Defend, result: Outcome::South },
    Battle { name: "Stones River", date: "December 31, 1862 - January 2, 1863", about: "Bragg falls on Rosecrans outside Murfreesboro and bends his line back, but cannot break it. After a failed attack on the third day, Bragg withdraws.", south_men: 35_000, north_men: 41_400, south_lost: 11_740, north_lost: 12_900, role: Role::Attack, result: Outcome::North },
    Battle { name: "Chancellorsville", date: "April 30 - May 6, 1863", about: "Outnumbered more than two to one, Lee splits his army and sends Jackson around Hooker's open flank. A great victory, and a costly one: Jackson is shot by his own men.", south_men: 61_000, north_men: 134_000, south_lost: 13_300, north_lost: 17_200, role: Role::Both, result: Outcome::South },
    Battle { name: "Vicksburg", date: "May 18 - July 4, 1863", about: "Grant besieges Pemberton in the fortress on the Mississippi. After six weeks of hunger and shelling the garrison surrenders, and the river belongs to the Union.", south_men: 33_000, north_men: 77_000, south_lost: 32_700, north_lost: 4_900, role: Role::Defend, result: Outcome::North },
    Battle { name: "Gettysburg", date: "July 1-3, 1863", about: "Lee's second invasion of the North. Three days of fighting end with Pickett's charge across open fields into Meade's centre, and Lee's retreat.", south_men: 71_700, north_men: 93_900, south_lost: 23_230, north_lost: 23_050, role: Role::Attack, result: Outcome::North },
    Battle { name: "Chickamauga", date: "September 18-20, 1863", about: "Reinforced by Longstreet, Bragg strikes Rosecrans in the woods of north Georgia. A gap opens in the Union line at the worst moment, and only Thomas's stand saves the army.", south_men: 65_000, north_men: 60_000, south_lost: 18_450, north_lost: 16_170, role: Role::Attack, result: Outcome::South },
    Battle { name: "Chattanooga", date: "November 23-25, 1863", about: "Grant breaks the siege of Chattanooga. Union soldiers storm up Missionary Ridge without orders and Bragg's army flees into Georgia.", south_men: 44_000, north_men: 56_000, south_lost: 6_670, north_lost: 5_820, role: Role::Defend, result: Outcome::North },
    Battle { name: "Spotsylvania", date: "May 8-21, 1864", about: "Grant attacks Lee's entrenchments for two weeks, the worst of it at the Bloody Angle. Neither side gives way, and Grant slides south toward Richmond.", south_men: 52_000, north_men: 100_000, south_lost: 12_000, north_lost: 18_400, role: Role::Defend, result: Outcome::Drawn },
    Battle { name: "Atlanta", date: "July 22, 1864", about: "Hood, new in command, swings around Sherman's flank east of Atlanta. The attack is beaten off, and six weeks later the city falls.", south_men: 40_000, north_men: 35_000, south_lost: 5_500, north_lost: 3_650, role: Role::Attack, result: Outcome::North },
];

pub const ATTACKS: [&str; 4] = [
    "Artillery bombardment",
    "Frontal assault",
    "Flanking march",
    "Encirclement",
];
pub const DEFENCES: [&str; 4] = [
    "Artillery barrage",
    "Dig in against assault",
    "Guard the flanks",
    "Fall back",
];

/// Money for a battle: about $4.50 for each man.
pub fn budget(men: u32) -> u32 {
    (men as f64 * 4.5 / 100.0).round() as u32 * 100
}

/// Inflation in per cent before battle `i` (0-based): the South's prices
/// climb, the North's ease.
pub fn inflation(south: bool, i: usize) -> f64 {
    if south {
        25.0 + 2.0 * i as f64
    } else {
        10.0 - 2.0 * i as f64
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Supply {
    pub food: u32,
    pub pay: u32,
    pub ammo: u32,
}

impl Supply {
    pub fn total(&self) -> u32 {
        self.food + self.pay + self.ammo
    }

    /// An even, sensible share of a budget.
    pub fn even(budget: u32) -> Supply {
        let food = budget * 35 / 100;
        let pay = budget * 30 / 100;
        Supply {
            food,
            pay,
            ammo: budget - food - pay,
        }
    }
}

/// 0.0 (wretched) to 1.0 (fine), from how well the men are fed and paid.
pub fn morale(s: &Supply, men: u32, infl: f64) -> f64 {
    let real = (s.food + s.pay) as f64 / (1.0 + infl / 100.0);
    let need = men as f64 * 2.4;
    (real / need).clamp(0.0, 1.5) / 1.5
}

pub fn morale_word(m: f64) -> &'static str {
    if m < 0.45 {
        "poor"
    } else if m < 0.7 {
        "fair"
    } else {
        "high"
    }
}

/// How well strategy `mine` (as attacker or defender) fares against
/// `theirs`: below 1 means fewer losses for me.
pub fn matchup(attacking: bool, mine: usize, theirs: usize) -> f64 {
    // rows: my choice; columns: theirs. Attacker's view: against the
    // defences (barrage, dig in, guard flanks, fall back).
    const ATT: [[f64; 4]; 4] = [
        [1.0, 0.85, 0.9, 0.7],   // bombardment
        [1.0, 1.45, 0.8, 0.75],  // frontal assault
        [0.85, 0.8, 1.35, 0.8],  // flanking march
        [1.05, 1.1, 1.15, 0.65], // encirclement
    ];
    // defender's view: against the attacks (bombard, frontal, flank, encircle)
    const DEF: [[f64; 4]; 4] = [
        [0.95, 0.9, 1.0, 1.0],  // barrage
        [1.1, 0.6, 1.25, 1.0],  // dig in against assault
        [1.05, 1.2, 0.65, 0.9], // guard the flanks
        [0.6, 0.6, 0.6, 0.55],  // fall back
    ];
    if attacking {
        ATT[mine][theirs]
    } else {
        DEF[mine][theirs]
    }
}

#[derive(Clone, Debug)]
pub struct Fought {
    pub battle: usize,
    pub south_lost: u32,
    pub north_lost: u32,
    pub south_deserted: u32,
    pub north_deserted: u32,
    pub south_choice: usize,
    pub north_choice: usize,
    pub result: Outcome,
}

/// Fight battle `i`. The South attacks unless history had it defending.
pub fn fight(
    i: usize,
    south: &Supply,
    south_choice: usize,
    north: &Supply,
    north_choice: usize,
    rng: &mut Rng,
) -> Fought {
    let b = &BATTLES[i];
    let south_attacks = b.role != Role::Defend;
    let north_attacks = b.role != Role::Attack;
    let sm = morale(south, b.south_men, inflation(true, i));
    let nm = morale(north, b.north_men, inflation(false, i));
    // ammunition per man against what an even share would buy
    let ammo_k = |s: &Supply, men: u32, infl: f64| {
        let real = s.ammo as f64 / (1.0 + infl / 100.0);
        (real / (men as f64 * 1.6)).clamp(0.2, 1.6)
    };
    let sa = ammo_k(south, b.south_men, inflation(true, i));
    let na = ammo_k(north, b.north_men, inflation(false, i));
    // losses: history's, scaled by the matchup, the enemy's fire and our spirit
    let s_fac = matchup(south_attacks, south_choice, north_choice)
        * (0.6 + 0.4 * na)
        * (1.25 - 0.35 * sm)
        * rng.between(0.88, 1.12) as f64;
    let n_fac = matchup(north_attacks, north_choice, south_choice)
        * (0.6 + 0.4 * sa)
        * (1.25 - 0.35 * nm)
        * rng.between(0.88, 1.12) as f64;
    let south_lost = ((b.south_lost as f64 * s_fac).round() as u32).min(b.south_men);
    let north_lost = ((b.north_lost as f64 * n_fac).round() as u32).min(b.north_men);
    let south_deserted = (b.south_men as f64 * (0.06 * (1.0 - sm)).max(0.0)).round() as u32;
    let north_deserted = (b.north_men as f64 * (0.06 * (1.0 - nm)).max(0.0)).round() as u32;
    // the side that loses the smaller share of its army carries the field;
    // falling back always gives it up
    let s_share = (south_lost + south_deserted) as f64 / b.south_men as f64;
    let n_share = (north_lost + north_deserted) as f64 / b.north_men as f64;
    let s_fled = !south_attacks && south_choice == 3;
    let n_fled = !north_attacks && north_choice == 3;
    let result = if s_fled && !n_fled {
        Outcome::North
    } else if n_fled && !s_fled {
        Outcome::South
    } else if (s_share - n_share).abs() < 0.01 {
        Outcome::Drawn
    } else if s_share < n_share {
        Outcome::South
    } else {
        Outcome::North
    };
    Fought {
        battle: i,
        south_lost,
        north_lost,
        south_deserted,
        north_deserted,
        south_choice,
        north_choice,
        result,
    }
}

/// The computer general: sensible supplies, and a plan picked with some
/// thought for what the South did last time.
pub fn north_plan(i: usize, last_south: Option<usize>, rng: &mut Rng) -> (Supply, usize) {
    let b = &BATTLES[i];
    let money = budget(b.north_men);
    let s = Supply::even(money);
    let attacking = b.role != Role::Attack;
    let choice = if attacking {
        // against a South that dug in last time, go round; otherwise mix it up
        match last_south {
            Some(1) if rng.chance(0.6) => 2,
            Some(2) if rng.chance(0.6) => 1,
            _ => rng.range(0, 3) as usize,
        }
    } else {
        match last_south {
            Some(1) if rng.chance(0.6) => 1,
            Some(2) if rng.chance(0.6) => 2,
            _ => rng.range(0, 2) as usize,
        }
    };
    (s, choice)
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Phase {
    Briefing,
    Supply,
    Plan,
    /// in a two-general game, the Union's turn
    NorthSupply,
    NorthPlan,
    Result,
    WarOver,
}

pub struct CivilWar {
    ctx: Ctx,
    two: bool,
    battle: usize,
    phase: Phase,
    fields: [NumField; 3],
    focus: usize,
    fresh: bool,
    choice: usize,
    south: Supply,
    south_choice: usize,
    north: Supply,
    north_choice: usize,
    fought: Vec<Fought>,
    note: String,
    quit: bool,
    rows: Vec<(Rect, usize)>,
    demo_clock: f32,
}

impl CivilWar {
    fn start_war(&mut self) {
        self.battle = 0;
        self.fought.clear();
        self.phase = Phase::Briefing;
        log::info!("new war, two generals {}", self.two);
    }

    fn side_men(&self, south: bool) -> u32 {
        let b = &BATTLES[self.battle];
        if south {
            b.south_men
        } else {
            b.north_men
        }
    }

    fn open_supply(&mut self, south: bool) {
        let money = budget(self.side_men(south));
        let even = Supply::even(money);
        self.fields = [
            NumField::new("Food", even.food as i64, 0, money as i64, 500),
            NumField::new("Pay", even.pay as i64, 0, money as i64, 500),
            NumField::new("Ammunition", even.ammo as i64, 0, money as i64, 500),
        ];
        self.focus = 0;
        self.fresh = true;
        self.phase = if south {
            Phase::Supply
        } else {
            Phase::NorthSupply
        };
    }

    fn supply_from_fields(&self) -> Supply {
        Supply {
            food: self.fields[0].value().max(0) as u32,
            pay: self.fields[1].value().max(0) as u32,
            ammo: self.fields[2].value().max(0) as u32,
        }
    }

    fn confirm_supply(&mut self) {
        let south = self.phase == Phase::Supply;
        let s = self.supply_from_fields();
        let money = budget(self.side_men(south));
        if s.total() > money {
            self.note = format!("That is ${}; the treasury has ${money}.", s.total());
            self.ctx.audio.play(Sound::Nope);
            return;
        }
        self.note.clear();
        if south {
            self.south = s;
            self.phase = Phase::Plan;
        } else {
            self.north = s;
            self.phase = Phase::NorthPlan;
        }
        self.choice = 0;
        self.ctx.audio.play(Sound::Click);
    }

    fn confirm_plan(&mut self) {
        if self.phase == Phase::Plan {
            self.south_choice = self.choice;
            if self.two {
                self.open_supply(false);
                return;
            }
            let last = self.fought.last().map(|f| f.south_choice);
            let (s, c) = north_plan(self.battle, last, &mut self.ctx.rng);
            self.north = s;
            self.north_choice = c;
        } else {
            self.north_choice = self.choice;
        }
        let f = fight(
            self.battle,
            &self.south,
            self.south_choice,
            &self.north,
            self.north_choice,
            &mut self.ctx.rng,
        );
        log::info!("battle {} {:?}", BATTLES[self.battle].name, f);
        self.ctx.audio.play(Sound::Boom);
        self.ctx.audio.play(match f.result {
            Outcome::South => Sound::Good,
            Outcome::North => Sound::Bad,
            Outcome::Drawn => Sound::Click,
        });
        self.fought.push(f);
        self.phase = Phase::Result;
    }

    fn advance(&mut self) {
        match self.phase {
            Phase::Briefing => self.open_supply(true),
            Phase::Supply | Phase::NorthSupply => self.confirm_supply(),
            Phase::Plan | Phase::NorthPlan => self.confirm_plan(),
            Phase::Result => {
                if self.battle + 1 >= BATTLES.len() {
                    self.phase = Phase::WarOver;
                    let (s, n) = self.tally();
                    self.ctx
                        .audio
                        .play(if s > n { Sound::Win } else { Sound::Lose });
                } else {
                    self.battle += 1;
                    self.phase = Phase::Briefing;
                }
            }
            Phase::WarOver => self.start_war(),
        }
    }

    fn tally(&self) -> (usize, usize) {
        let s = self
            .fought
            .iter()
            .filter(|f| f.result == Outcome::South)
            .count();
        let n = self
            .fought
            .iter()
            .filter(|f| f.result == Outcome::North)
            .count();
        (s, n)
    }

    fn attacking(&self, south: bool) -> bool {
        let r = BATTLES[self.battle].role;
        if south {
            r != Role::Defend
        } else {
            r != Role::Attack
        }
    }

    fn draw_briefing(&self, buf: &mut Buffer, r: Rect) {
        let b = &BATTLES[self.battle];
        let p = panel(
            buf,
            r,
            &format!("{}/{} {}", self.battle + 1, BATTLES.len(), b.name),
            AMBER,
        );
        put(buf, p.x, p.y, b.date, st(DIM));
        let used = para(
            buf,
            Rect {
                y: p.y + 1,
                height: p.height.saturating_sub(1),
                ..p
            },
            b.about,
            st(INK),
        );
        let y = p.y + 2 + used;
        let i = self.battle;
        let rows = [
            ("", "South".to_string(), "North".to_string()),
            ("Men", fmt_n(b.south_men), fmt_n(b.north_men)),
            (
                "Money",
                format!("${}", fmt_n(budget(b.south_men))),
                format!("${}", fmt_n(budget(b.north_men))),
            ),
            (
                "Inflation",
                format!("{:.0}%", inflation(true, i)),
                format!("{:.0}%", inflation(false, i)),
            ),
        ];
        for (k, (a, s, n)) in rows.iter().enumerate() {
            let yy = y + k as u16;
            if yy < p.bottom() {
                let style = if k == 0 { bold(st(DIM)) } else { st(INK) };
                put(buf, p.x, yy, a, st(DIM));
                put(
                    buf,
                    p.x + 10,
                    yy,
                    &format!("{s:>8}"),
                    if k == 0 {
                        bold(st(Color::Rgb(170, 170, 190)))
                    } else {
                        style
                    },
                );
                put(
                    buf,
                    p.x + 19,
                    yy,
                    &format!("{n:>8}"),
                    if k == 0 { bold(st(BLUE)) } else { style },
                );
            }
        }
        let yy = y + 5;
        if yy < p.bottom() {
            let role = match b.role {
                Role::Attack => "The South is on the attack.",
                Role::Defend => "The South is on the defence.",
                Role::Both => "Both armies are on the attack.",
            };
            put(buf, p.x, yy, role, bold(st(AMBER)));
        }
    }

    fn draw_orders(&mut self, buf: &mut Buffer, r: Rect) {
        let south = matches!(self.phase, Phase::Supply | Phase::Plan);
        let side = if south { "South" } else { "North" };
        let title = match self.phase {
            Phase::Result => "Battle report".to_string(),
            Phase::WarOver => "The war is over".to_string(),
            Phase::Briefing => "Orders".to_string(),
            _ => format!("Orders for the {side}"),
        };
        let p = panel(
            buf,
            r,
            &title,
            if south || !matches!(self.phase, Phase::NorthSupply | Phase::NorthPlan) {
                INK
            } else {
                BLUE
            },
        );
        self.rows.clear();
        match self.phase {
            Phase::Briefing => {
                para(buf, p, "Read the situation, then press Enter to spend the army's money: food and pay keep the men in good heart, ammunition makes their fire tell. Then choose how to fight.", st(DIM));
            }
            Phase::Supply | Phase::NorthSupply => {
                let men = self.side_men(south);
                let money = budget(men);
                let s = self.supply_from_fields();
                put_max(
                    buf,
                    p.x,
                    p.y,
                    &format!("${} of ${}", fmt_n(s.total()), fmt_n(money)),
                    st(if s.total() > money { RED } else { INK }),
                    p.width,
                );
                for (k, f) in self.fields.iter().enumerate() {
                    let y = p.y + 2 + k as u16 * 2;
                    f.draw(buf, p.x + 2, y, 14, 10, k == self.focus);
                    self.rows.push((Rect::new(p.x, y, p.width, 1), k));
                }
                let m = morale(&s, men, inflation(south, self.battle));
                let y = p.y + 9;
                put_max(
                    buf,
                    p.x,
                    y,
                    &format!("Morale would be {}", morale_word(m)),
                    st(match morale_word(m) {
                        "poor" => RED,
                        "fair" => AMBER,
                        _ => GREEN,
                    }),
                    p.width,
                );
                bar(buf, p.x, y + 1, p.width.min(30), m as f32, GREEN);
                if !self.note.is_empty() {
                    para(
                        buf,
                        Rect {
                            y: y + 3,
                            height: p.bottom().saturating_sub(y + 3),
                            ..p
                        },
                        &self.note,
                        st(RED),
                    );
                }
            }
            Phase::Plan | Phase::NorthPlan => {
                let att = self.attacking(south);
                let names = if att { ATTACKS } else { DEFENCES };
                put_max(
                    buf,
                    p.x,
                    p.y,
                    if att {
                        "How will you attack?"
                    } else {
                        "How will you defend?"
                    },
                    st(INK),
                    p.width,
                );
                let items: Vec<String> = names
                    .iter()
                    .enumerate()
                    .map(|(k, n)| format!("{} {n}", k + 1))
                    .collect();
                let rows = menu(
                    buf,
                    Rect {
                        y: p.y + 2,
                        height: 4,
                        ..p
                    },
                    &items,
                    self.choice,
                );
                for (k, row) in rows.into_iter().enumerate() {
                    self.rows.push((row, k));
                }
                let hint = if att {
                    "An assault is murder against dug-in men; a flanking march fails against a guarded flank."
                } else {
                    "Digging in punishes an assault; guarding the flanks stops a march around them. Falling back saves lives but gives up the field."
                };
                para(
                    buf,
                    Rect {
                        y: p.y + 7,
                        height: p.height.saturating_sub(7),
                        ..p
                    },
                    hint,
                    st(DIM),
                );
            }
            Phase::Result => {
                let f = self.fought.last().unwrap();
                let b = &BATTLES[f.battle];
                let sa = self.attacking(true);
                let s_name = if sa {
                    ATTACKS[f.south_choice]
                } else {
                    DEFENCES[f.south_choice]
                };
                let n_name = if b.role != Role::Attack {
                    ATTACKS[f.north_choice]
                } else {
                    DEFENCES[f.north_choice]
                };
                put_max(buf, p.x, p.y, &format!("South: {s_name}"), st(INK), p.width);
                put_max(
                    buf,
                    p.x,
                    p.y + 1,
                    &format!("North: {n_name}"),
                    st(BLUE),
                    p.width,
                );
                put_max(
                    buf,
                    p.x,
                    p.y + 3,
                    "            South   North",
                    st(DIM),
                    p.width,
                );
                put_max(
                    buf,
                    p.x,
                    p.y + 4,
                    &format!(
                        "lost     {:>7} {:>7}",
                        fmt_n(f.south_lost),
                        fmt_n(f.north_lost)
                    ),
                    st(INK),
                    p.width,
                );
                put_max(
                    buf,
                    p.x,
                    p.y + 5,
                    &format!(
                        "deserted {:>7} {:>7}",
                        fmt_n(f.south_deserted),
                        fmt_n(f.north_deserted)
                    ),
                    st(INK),
                    p.width,
                );
                put_max(
                    buf,
                    p.x,
                    p.y + 6,
                    &format!(
                        "history  {:>7} {:>7}",
                        fmt_n(b.south_lost),
                        fmt_n(b.north_lost)
                    ),
                    st(DIM),
                    p.width,
                );
                let pct = f.south_lost as f64 / b.south_lost as f64 * 100.0;
                put_max(
                    buf,
                    p.x,
                    p.y + 8,
                    &format!("South lost {pct:.0}% of history's toll."),
                    st(INK),
                    p.width,
                );
                let (msg, col) = match f.result {
                    Outcome::South => ("The South holds the field.", GREEN),
                    Outcome::North => ("The North holds the field.", RED),
                    Outcome::Drawn => ("Neither side gives way: a draw.", AMBER),
                };
                put_max(buf, p.x, p.y + 10, msg, bold(st(col)), p.width);
                let hist = match b.result {
                    Outcome::South => "In history, the South won.",
                    Outcome::North => "In history, the North won.",
                    Outcome::Drawn => "In history, neither side won.",
                };
                put_max(buf, p.x, p.y + 11, hist, st(DIM), p.width);
            }
            Phase::WarOver => {
                let (s, n) = self.tally();
                let d = self.fought.len() - s - n;
                let lost: u32 = self.fought.iter().map(|f| f.south_lost).sum();
                let hist: u32 = BATTLES.iter().map(|b| b.south_lost).sum();
                let text = format!(
                    "The war is refought. The South won {s} battles, the North {n}, and {d} were drawn; history gave the South six. Southern losses came to {} men against history's {}.\n\nEnter fights it again.",
                    fmt_n(lost),
                    fmt_n(hist)
                );
                para(buf, p, &text, st(INK));
            }
        }
    }

    fn draw_campaign(&self, buf: &mut Buffer, r: Rect) {
        let p = panel(buf, r, "The war", CYAN);
        put(buf, p.x, p.y, "battle           then now", st(DIM));
        for (i, b) in BATTLES.iter().enumerate() {
            let y = p.y + 1 + i as u16;
            if y >= p.bottom() {
                break;
            }
            let mark = |o: Outcome| match o {
                Outcome::South => ("S", Color::Rgb(170, 170, 190)),
                Outcome::North => ("N", BLUE),
                Outcome::Drawn => ("=", AMBER),
            };
            let current = i == self.battle && self.phase != Phase::WarOver;
            let name: String = b.name.chars().take(17).collect();
            put(
                buf,
                p.x,
                y,
                &format!("{name:<17}"),
                if current { bold(st(AMBER)) } else { st(INK) },
            );
            let (t, c) = mark(b.result);
            put(buf, p.x + 18, y, t, st(c));
            if let Some(f) = self.fought.iter().find(|f| f.battle == i) {
                let (t, c) = mark(f.result);
                put(buf, p.x + 22, y, t, bold(st(c)));
            }
        }
        let (s, n) = self.tally();
        let y = p.y + 2 + BATTLES.len() as u16;
        if y < p.bottom() {
            put(buf, p.x, y, &format!("South {s}, North {n}"), st(DIM));
        }
    }
}

use ratatui::style::Color;

pub fn fmt_n(n: u32) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

impl Game for CivilWar {
    const ID: &'static str = "civil-war";
    const TITLE: &'static str = "Civil War";
    const HELP: &'static str = "Fourteen battles of the American Civil War, from First Bull Run to
Atlanta, with the armies history brought to each. You supply and command
the South: share out the treasury between food, pay and ammunition, then
choose how to attack or defend. The computer, or a second player at the
same keyboard (start with --two), commands the North. Each result is set
against what really happened.
Enter goes on, Tab or Up/Down move, typing changes a number, 1-4 pick a plan.";

    fn new(ctx: Ctx) -> Self {
        let two = std::env::args().any(|a| a == "--two");
        let mut g = CivilWar {
            ctx,
            two,
            battle: 0,
            phase: Phase::Briefing,
            fields: [
                NumField::new("", 0, 0, 0, 1),
                NumField::new("", 0, 0, 0, 1),
                NumField::new("", 0, 0, 0, 1),
            ],
            focus: 0,
            fresh: true,
            choice: 0,
            south: Supply::default(),
            south_choice: 0,
            north: Supply::default(),
            north_choice: 0,
            fought: Vec::new(),
            note: String::new(),
            quit: false,
            rows: Vec::new(),
            demo_clock: 0.0,
        };
        g.start_war();
        g
    }

    fn key(&mut self, k: Key) {
        if self.ctx.demo {
            if matches!(k, Key::Esc | Key::Char('q')) {
                self.quit = true;
            }
            return;
        }
        match self.phase {
            Phase::Supply | Phase::NorthSupply => match k {
                Key::Esc => self.quit = true,
                Key::Enter => self.advance(),
                Key::Tab | Key::Right => {
                    self.focus = (self.focus + 1) % 3;
                    self.fresh = true;
                }
                Key::BackTab | Key::Left => {
                    self.focus = (self.focus + 2) % 3;
                    self.fresh = true;
                }
                other => {
                    let mut fresh = self.fresh;
                    if self.fields[self.focus].key(&other, &mut fresh) {
                        self.fresh = fresh;
                    }
                }
            },
            Phase::Plan | Phase::NorthPlan => match k {
                Key::Esc | Key::Char('q') => self.quit = true,
                Key::Up => self.choice = (self.choice + 3) % 4,
                Key::Down | Key::Tab => self.choice = (self.choice + 1) % 4,
                Key::Char(c @ '1'..='4') => {
                    self.choice = (c as u8 - b'1') as usize;
                    self.advance();
                }
                Key::Enter => self.advance(),
                _ => {}
            },
            _ => match k {
                Key::Esc | Key::Char('q') => self.quit = true,
                Key::Char('m') => {
                    self.ctx.audio.toggle();
                    crate::shell::remember_mute(Self::ID, self.ctx.audio.muted);
                }
                Key::Enter | Key::Char(' ') => self.advance(),
                _ => {}
            },
        }
    }

    fn mouse(&mut self, m: Mouse) {
        if m.kind != MouseKind::Click {
            return;
        }
        if let Some(&(_, k)) = self.rows.iter().find(|(r, _)| hit(r, m.x, m.y)) {
            match self.phase {
                Phase::Plan | Phase::NorthPlan => {
                    self.choice = k;
                    self.advance();
                }
                Phase::Supply | Phase::NorthSupply => {
                    self.focus = k;
                    self.fresh = true;
                }
                _ => {}
            }
        } else if matches!(self.phase, Phase::Briefing | Phase::Result | Phase::WarOver) {
            self.advance();
        }
    }

    fn tick(&mut self, dt: f32) {
        if !self.ctx.demo {
            return;
        }
        self.demo_clock += dt;
        let wait = match self.phase {
            Phase::Briefing => 3.0,
            Phase::Result => 3.5,
            _ => 1.6,
        };
        if self.demo_clock < wait {
            return;
        }
        self.demo_clock = 0.0;
        match self.phase {
            Phase::Supply => {
                // a quartermaster who feeds and pays a little over the odds
                let money = budget(self.side_men(true)) as i64;
                self.fields[0].set(money * 38 / 100);
                self.fields[1].set(money * 32 / 100);
                self.fields[2].set(money * 30 / 100);
                self.advance();
            }
            Phase::Plan => {
                // dig in when defending, go round when attacking
                self.choice = if self.attacking(true) {
                    2
                } else {
                    1 + self.ctx.rng.range(0, 1) as usize
                };
                self.advance();
            }
            _ => self.advance(),
        }
    }

    fn quit(&self) -> bool {
        self.quit
    }

    fn snapshot(&self) -> String {
        format!(
            "battle {} phase {:?} choice {} focus {} fought {}",
            self.battle,
            self.phase,
            self.choice,
            self.focus,
            self.fought.len()
        )
    }

    fn draw(&mut self, area: Rect, buf: &mut Buffer) {
        let (s, n) = self.tally();
        let status = format!("South {s}  North {n}");
        let keys: &[(&str, &str)] = match self.phase {
            Phase::Supply | Phase::NorthSupply => &[
                ("Tab", "next"),
                ("↑↓", "±$500"),
                ("0-9", "type"),
                ("Enter", "done"),
                ("Esc", "quit"),
            ],
            Phase::Plan | Phase::NorthPlan => &[
                ("1-4", "choose"),
                ("↑↓", "move"),
                ("Enter", "fight"),
                ("Esc", "quit"),
            ],
            _ => &[("Enter", "go on"), ("M", "sound"), ("Esc", "quit")],
        };
        let inner = frame(buf, area, "Civil War", &status, keys);
        if too_small(buf, inner, 90, 24) {
            return;
        }
        let camp_w = 28;
        let left_w = (inner.width - camp_w) * 45 / 100;
        let left = Rect {
            width: left_w,
            ..inner
        };
        let mid = Rect {
            x: inner.x + left_w,
            width: inner.width - camp_w - left_w,
            ..inner
        };
        let right = Rect {
            x: inner.right() - camp_w,
            width: camp_w,
            ..inner
        };
        self.draw_briefing(buf, left);
        self.draw_orders(buf, mid);
        self.draw_campaign(buf, right);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fourteen_battles_with_sane_figures() {
        assert_eq!(BATTLES.len(), 14);
        for b in BATTLES {
            assert!(
                b.south_lost < b.south_men && b.north_lost < b.north_men,
                "{}",
                b.name
            );
        }
    }

    #[test]
    fn following_history_gives_history_like_losses() {
        let mut rng = Rng::new(3);
        let b = &BATTLES[5]; // Fredericksburg: dig in against a frontal assault
        let money = budget(b.south_men);
        let f = fight(
            5,
            &Supply::even(money),
            1,
            &Supply::even(budget(b.north_men)),
            1,
            &mut rng,
        );
        assert_eq!(f.result, Outcome::South);
        assert!(f.south_lost < b.south_lost);
    }

    #[test]
    fn falling_back_gives_up_the_field() {
        let mut rng = Rng::new(5);
        let i = 0; // the South defends at First Bull Run
        let s = Supply::even(budget(BATTLES[i].south_men));
        let n = Supply::even(budget(BATTLES[i].north_men));
        assert_eq!(fight(i, &s, 3, &n, 1, &mut rng).result, Outcome::North);
    }

    #[test]
    fn money_and_numbers() {
        assert_eq!(budget(18_000), 81_000);
        assert_eq!(fmt_n(1_234_567), "1,234,567");
        assert_eq!(inflation(true, 0), 25.0);
        assert_eq!(inflation(false, 3), 4.0);
    }
}
