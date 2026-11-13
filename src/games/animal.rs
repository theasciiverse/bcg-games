//! Animal: think of an animal and the computer guesses it with yes-or-no
//! questions, learning a new question each time it is wrong. Rules in
//! asciiverse/specs/animal.md.

use crate::audio::Sound;
use crate::shell::{Ctx, Game, Key, Mouse, MouseKind};
use crate::ui::*;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

/// What the computer knows: a tree of questions with animals at the leaves.
#[derive(Clone, Debug, PartialEq)]
pub enum Node {
    Animal(String),
    Question {
        text: String,
        yes: Box<Node>,
        no: Box<Node>,
    },
}

impl Node {
    pub fn starter() -> Node {
        Node::Question {
            text: "Does it live in water".into(),
            yes: Box::new(Node::Animal("fish".into())),
            no: Box::new(Node::Animal("bird".into())),
        }
    }

    pub fn animals(&self) -> Vec<String> {
        match self {
            Node::Animal(a) => vec![a.clone()],
            Node::Question { yes, no, .. } => {
                let mut v = yes.animals();
                v.extend(no.animals());
                v
            }
        }
    }

    /// The node at a path of answers from the root (true = yes).
    pub fn at(&self, path: &[bool]) -> &Node {
        let mut n = self;
        for &a in path {
            if let Node::Question { yes, no, .. } = n {
                n = if a { yes } else { no };
            }
        }
        n
    }

    fn at_mut(&mut self, path: &[bool]) -> &mut Node {
        let mut n = self;
        for &a in path {
            if let Node::Question { yes, no, .. } = n {
                n = if a { yes } else { no };
            }
        }
        n
    }

    /// Replace the guess at `path` with a question telling `new` from it.
    pub fn learn(&mut self, path: &[bool], new: &str, question: &str, new_is_yes: bool) {
        let leaf = self.at_mut(path);
        let old = std::mem::replace(leaf, Node::Animal(String::new()));
        let fresh = Box::new(Node::Animal(new.to_string()));
        let (yes, no) = if new_is_yes {
            (fresh, Box::new(old))
        } else {
            (Box::new(old), fresh)
        };
        *leaf = Node::Question {
            text: question.to_string(),
            yes,
            no,
        };
    }

    /// One line per node, preorder: `?question` or `=animal`.
    pub fn save(&self) -> String {
        let mut out = String::new();
        fn go(n: &Node, out: &mut String) {
            match n {
                Node::Animal(a) => {
                    out.push('=');
                    out.push_str(a);
                    out.push('\n');
                }
                Node::Question { text, yes, no } => {
                    out.push('?');
                    out.push_str(text);
                    out.push('\n');
                    go(yes, out);
                    go(no, out);
                }
            }
        }
        go(self, &mut out);
        out
    }

    pub fn load(text: &str) -> Option<Node> {
        fn go<'a>(lines: &mut impl Iterator<Item = &'a str>) -> Option<Node> {
            let l = lines.next()?;
            if let Some(a) = l.strip_prefix('=') {
                Some(Node::Animal(a.to_string()))
            } else if let Some(q) = l.strip_prefix('?') {
                let yes = go(lines)?;
                let no = go(lines)?;
                Some(Node::Question {
                    text: q.to_string(),
                    yes: Box::new(yes),
                    no: Box::new(no),
                })
            } else {
                None
            }
        }
        let mut it = text.lines().filter(|l| !l.is_empty());
        let n = go(&mut it)?;
        it.next().is_none().then_some(n)
    }

    /// The tree as indented lines for display, with each line's path.
    pub fn outline(&self) -> Vec<(usize, String, bool, Vec<bool>)> {
        let mut out = Vec::new();
        fn go(
            n: &Node,
            depth: usize,
            path: &mut Vec<bool>,
            out: &mut Vec<(usize, String, bool, Vec<bool>)>,
        ) {
            match n {
                Node::Animal(a) => out.push((depth, a.clone(), true, path.clone())),
                Node::Question { text, yes, no } => {
                    out.push((depth, format!("{text}?"), false, path.clone()));
                    path.push(true);
                    go(yes, depth + 1, path, out);
                    path.pop();
                    path.push(false);
                    go(no, depth + 1, path, out);
                    path.pop();
                }
            }
        }
        go(self, 0, &mut Vec::new(), &mut out);
        out
    }
}

pub fn article(name: &str) -> &'static str {
    match name.chars().next().map(|c| c.to_ascii_lowercase()) {
        Some('a' | 'e' | 'i' | 'o' | 'u') => "an",
        _ => "a",
    }
}

/// Tidy what the player typed: no question mark, a capital first letter.
pub fn tidy_question(q: &str) -> String {
    let q = q.trim().trim_end_matches('?').trim();
    let mut c = q.chars();
    match c.next() {
        Some(f) => f.to_uppercase().chain(c).collect(),
        None => String::new(),
    }
}

pub fn tidy_animal(a: &str) -> String {
    let a = a.trim().to_lowercase();
    let a = a
        .strip_prefix("a ")
        .or_else(|| a.strip_prefix("an "))
        .unwrap_or(&a)
        .trim()
        .to_string();
    a
}

#[derive(Clone, Debug, PartialEq)]
enum Phase {
    Ready,
    Asking,
    Guessing,
    Won,
    WhatWasIt,
    WhatQuestion,
    WhichAnswer,
    Learned,
}

/// The demo's own knowledge, to answer the computer's questions.
const DEMO_ANIMALS: &[(&str, &[(&str, bool)])] = &[
    (
        "elephant",
        &[
            ("Does it live in water", false),
            ("Does it have a trunk", true),
            ("Is it a mammal", true),
        ],
    ),
    (
        "shark",
        &[
            ("Does it live in water", true),
            ("Does it have sharp teeth", true),
            ("Is it a mammal", false),
        ],
    ),
    (
        "eagle",
        &[
            ("Does it live in water", false),
            ("Does it have a trunk", false),
            ("Does it hunt other animals", true),
            ("Is it a mammal", false),
        ],
    ),
    (
        "dolphin",
        &[
            ("Does it live in water", true),
            ("Does it have sharp teeth", false),
            ("Is it a mammal", true),
        ],
    ),
    (
        "cat",
        &[
            ("Does it live in water", false),
            ("Does it have a trunk", false),
            ("Does it hunt other animals", true),
            ("Is it a mammal", true),
        ],
    ),
    (
        "cow",
        &[
            ("Does it live in water", false),
            ("Does it have a trunk", false),
            ("Does it hunt other animals", false),
            ("Is it a mammal", true),
        ],
    ),
];

pub struct Animal {
    ctx: Ctx,
    tree: Node,
    path: Vec<bool>,
    phase: Phase,
    field: TextField,
    new_animal: String,
    new_question: String,
    show_tree: bool,
    scroll: usize,
    quit: bool,
    games: u32,
    buttons: Vec<(Rect, Key)>,
    demo_clock: f32,
    demo_pick: usize,
    demo_typing: Option<(String, usize)>,
}

impl Animal {
    fn save(&self) {
        if !self.ctx.demo {
            crate::store::write(Self::ID, "animals.txt", &self.tree.save());
        }
    }

    fn begin(&mut self) {
        self.path.clear();
        self.phase = Phase::Asking;
        self.ctx.audio.play(Sound::Click);
        self.settle();
    }

    /// From the current node: a question to ask, or a guess to make.
    fn settle(&mut self) {
        if let Node::Animal(_) = self.tree.at(&self.path) {
            self.phase = Phase::Guessing;
        }
    }

    fn answer(&mut self, yes: bool) {
        match self.phase {
            Phase::Asking => {
                self.path.push(yes);
                self.ctx.audio.play(Sound::Tick);
                self.settle();
            }
            Phase::Guessing => {
                self.games += 1;
                if yes {
                    self.phase = Phase::Won;
                    self.ctx.audio.play(Sound::Win);
                    log::info!("guessed right after {} questions", self.path.len());
                } else {
                    self.phase = Phase::WhatWasIt;
                    self.field = TextField::new(30);
                    self.ctx.audio.play(Sound::Bad);
                }
            }
            Phase::WhichAnswer => {
                let q = self.new_question.clone();
                let a = self.new_animal.clone();
                self.tree.learn(&self.path, &a, &q, yes);
                log::info!("learned {a:?} with {q:?} = {yes}");
                self.save();
                self.phase = Phase::Learned;
                self.ctx.audio.play(Sound::Good);
            }
            _ => {}
        }
    }

    fn submit_text(&mut self) {
        match self.phase {
            Phase::WhatWasIt => {
                let a = tidy_animal(&self.field.text);
                if a.is_empty() {
                    self.ctx.audio.play(Sound::Nope);
                    return;
                }
                self.new_animal = a;
                self.field = TextField::new(60);
                self.phase = Phase::WhatQuestion;
                self.ctx.audio.play(Sound::Click);
            }
            Phase::WhatQuestion => {
                let q = tidy_question(&self.field.text);
                if q.split_whitespace().count() < 2 {
                    self.ctx.audio.play(Sound::Nope);
                    return;
                }
                self.new_question = q;
                self.phase = Phase::WhichAnswer;
                self.ctx.audio.play(Sound::Click);
            }
            _ => {}
        }
    }

    fn guess_name(&self) -> String {
        match self.tree.at(&self.path) {
            Node::Animal(a) => a.clone(),
            _ => String::new(),
        }
    }

    fn typing(&self) -> bool {
        matches!(self.phase, Phase::WhatWasIt | Phase::WhatQuestion)
    }

    fn demo_answer(&self, animal: &str, question: &str) -> bool {
        let facts = DEMO_ANIMALS
            .iter()
            .find(|(a, _)| *a == animal)
            .map(|(_, f)| *f)
            .unwrap_or(&[]);
        if let Some((_, v)) = facts.iter().find(|(q, _)| *q == question) {
            return *v;
        }
        // questions the demo never taught: answer as for the closest thing
        false
    }

    fn demo_step(&mut self) {
        let animal = DEMO_ANIMALS[self.demo_pick % DEMO_ANIMALS.len()].0;
        if let Some((text, n)) = self.demo_typing.take() {
            let chars: Vec<char> = text.chars().collect();
            if n < chars.len() {
                self.field.key(&Key::Char(chars[n]));
                self.demo_typing = Some((text, n + 1));
            } else {
                self.submit_text();
            }
            return;
        }
        match self.phase {
            Phase::Ready => self.begin(),
            Phase::Asking => {
                let q = match self.tree.at(&self.path) {
                    Node::Question { text, .. } => text.clone(),
                    _ => String::new(),
                };
                let a = self.demo_answer(animal, &q);
                self.answer(a);
            }
            Phase::Guessing => {
                let g = self.guess_name();
                self.answer(g == animal);
            }
            Phase::WhatWasIt => self.demo_typing = Some((animal.to_string(), 0)),
            Phase::WhatQuestion => {
                // a question this animal answers differently from the guess
                let guess = self.guess_name();
                let facts = DEMO_ANIMALS
                    .iter()
                    .find(|(a, _)| *a == animal)
                    .map(|(_, f)| *f)
                    .unwrap_or(&[]);
                let q = facts
                    .iter()
                    .find(|(q, v)| {
                        self.demo_answer(&guess, q) != *v && !self.tree.save().contains(*q)
                    })
                    .map(|(q, _)| q.to_string())
                    .unwrap_or_else(|| format!("Is it {} {animal}", article(animal)));
                self.demo_typing = Some((q, 0));
            }
            Phase::WhichAnswer => {
                let q = self.new_question.clone();
                let facts = DEMO_ANIMALS
                    .iter()
                    .find(|(a, _)| *a == animal)
                    .map(|(_, f)| *f)
                    .unwrap_or(&[]);
                let v = facts.iter().find(|(x, _)| *x == q).is_none_or(|(_, v)| *v);
                self.answer(v);
            }
            Phase::Won | Phase::Learned => {
                self.demo_pick += 1;
                self.phase = Phase::Ready;
                if self.demo_pick.is_multiple_of(4) {
                    self.show_tree = !self.show_tree;
                }
            }
        }
    }
}

impl Game for Animal {
    const ID: &'static str = "animal";
    const TITLE: &'static str = "Animal";
    const HELP: &'static str = "Think of an animal. The computer asks yes-or-no questions and then
guesses. When it is wrong, tell it your animal and a question that tells
the two apart: it remembers, and slowly learns every animal you know.
It starts out knowing two. Y or N (or the arrows and Enter) to answer,
type and press Enter when asked, Tab shows everything it has learned.";

    fn new(ctx: Ctx) -> Self {
        let tree = if ctx.demo {
            Node::starter()
        } else {
            crate::store::read("animal", "animals.txt")
                .and_then(|t| Node::load(&t))
                .unwrap_or_else(Node::starter)
        };
        Animal {
            ctx,
            tree,
            path: Vec::new(),
            phase: Phase::Ready,
            field: TextField::new(30),
            new_animal: String::new(),
            new_question: String::new(),
            show_tree: false,
            scroll: 0,
            quit: false,
            games: 0,
            buttons: Vec::new(),
            demo_clock: 0.0,
            demo_pick: 0,
            demo_typing: None,
        }
    }

    fn key(&mut self, k: Key) {
        if self.ctx.demo {
            if matches!(k, Key::Esc | Key::Char('q')) {
                self.quit = true;
            }
            return;
        }
        if self.typing() {
            match k {
                Key::Enter => self.submit_text(),
                Key::Esc => {
                    self.phase = Phase::Ready;
                }
                other => {
                    self.field.key(&other);
                }
            }
            return;
        }
        match k {
            Key::Esc | Key::Char('q') => self.quit = true,
            Key::Tab | Key::Char('l') => self.show_tree = !self.show_tree,
            Key::Char('m') => {
                self.ctx.audio.toggle();
                crate::shell::remember_mute(Self::ID, self.ctx.audio.muted);
            }
            Key::PageDown | Key::Down if self.show_tree => self.scroll += 3,
            Key::PageUp | Key::Up if self.show_tree => self.scroll = self.scroll.saturating_sub(3),
            Key::Char('y') | Key::Left => self.answer(true),
            Key::Char('n') | Key::Right => self.answer(false),
            Key::Enter | Key::Char(' ') => match self.phase {
                Phase::Ready | Phase::Won | Phase::Learned => self.begin(),
                _ => {}
            },
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
            MouseKind::ScrollDown => self.scroll += 2,
            MouseKind::ScrollUp => self.scroll = self.scroll.saturating_sub(2),
            _ => {}
        }
    }

    fn tick(&mut self, dt: f32) {
        if !self.ctx.demo {
            return;
        }
        self.demo_clock += dt;
        let wait = if self.demo_typing.is_some() {
            0.07
        } else {
            1.1
        };
        if self.demo_clock >= wait {
            self.demo_clock = 0.0;
            self.demo_step();
        }
    }

    fn quit(&self) -> bool {
        self.quit
    }

    fn snapshot(&self) -> String {
        format!(
            "phase {:?} path {:?} animals {} tree {}",
            self.phase,
            self.path,
            self.tree.animals().len(),
            self.show_tree
        )
    }

    fn draw(&mut self, area: Rect, buf: &mut Buffer) {
        self.buttons.clear();
        let known = self.tree.animals().len();
        let status = format!("knows {known} animals");
        let keys: &[(&str, &str)] = if self.typing() {
            &[("type", ""), ("Enter", "done"), ("Esc", "give up")]
        } else {
            &[
                ("Y", "yes"),
                ("N", "no"),
                ("Enter", "play"),
                ("Tab", "what it knows"),
                ("M", "sound"),
                ("Esc", "quit"),
            ]
        };
        let inner = frame(buf, area, "Animal", &status, keys);
        if too_small(buf, inner, 64, 20) {
            return;
        }
        let side_w = if self.show_tree {
            inner.width / 2
        } else {
            30.min(inner.width / 3)
        };
        let main = Rect {
            width: inner.width - side_w,
            ..inner
        };
        let side = Rect {
            x: inner.x + main.width,
            width: side_w,
            ..inner
        };

        let p = panel(buf, main, "", AMBER);
        let guess = self.guess_name();
        let (big, small): (String, String) = match self.phase {
            Phase::Ready => (
                "Think of an animal.".into(),
                "Any animal at all. Press Enter when you have one in mind.".into(),
            ),
            Phase::Asking => match self.tree.at(&self.path) {
                Node::Question { text, .. } => (
                    format!("{text}?"),
                    format!("Question {}", self.path.len() + 1),
                ),
                _ => (String::new(), String::new()),
            },
            Phase::Guessing => (
                format!("Is it {} {guess}?", article(&guess)),
                "My guess.".into(),
            ),
            Phase::Won => (
                "I knew it!".into(),
                format!(
                    "{} {guess}. Enter for another round.",
                    match article(&guess) {
                        "an" => "An",
                        _ => "A",
                    }
                ),
            ),
            Phase::WhatWasIt => (
                "You win. What was it?".into(),
                "Type the animal you were thinking of.".into(),
            ),
            Phase::WhatQuestion => (
                format!(
                    "What would tell {} {} from {} {}?",
                    article(&self.new_animal),
                    self.new_animal,
                    article(&guess),
                    guess
                ),
                "Type a yes-or-no question about it.".into(),
            ),
            Phase::WhichAnswer => (
                format!("{}?", self.new_question),
                format!(
                    "And for {} {}, is the answer yes or no?",
                    article(&self.new_animal),
                    self.new_animal
                ),
            ),
            Phase::Learned => (
                "Thank you. I'll remember that.".into(),
                format!("Now I know {known} animals. Enter for another round."),
            ),
        };
        let mid = p.y + p.height / 2 - 3;
        let lines = wrap(&big, p.width.saturating_sub(4) as usize);
        for (i, l) in lines.iter().enumerate() {
            centre(
                buf,
                p,
                mid + i as u16,
                l,
                bold(st(match self.phase {
                    Phase::Won | Phase::Learned => GREEN,
                    Phase::WhatWasIt => AMBER,
                    _ => INK,
                })),
            );
        }
        let sy = mid + lines.len() as u16 + 1;
        for (i, l) in wrap(&small, p.width.saturating_sub(4) as usize)
            .iter()
            .enumerate()
        {
            centre(buf, p, sy + i as u16, l, st(DIM));
        }
        let by = sy + 3;
        if matches!(
            self.phase,
            Phase::Asking | Phase::Guessing | Phase::WhichAnswer
        ) {
            let yes = Rect::new(p.x + p.width / 2 - 13, by, 11, 1);
            let no = Rect::new(p.x + p.width / 2 + 2, by, 11, 1);
            fill(buf, yes, on(BG, GREEN));
            put(buf, yes.x + 2, by, "Y  yes", bold(on(BG, GREEN)));
            fill(buf, no, on(BG, RED));
            put(buf, no.x + 2, by, "N  no", bold(on(BG, RED)));
            self.buttons.push((yes, Key::Char('y')));
            self.buttons.push((no, Key::Char('n')));
        }
        if self.typing() {
            let w = p.width.saturating_sub(8).min(60);
            self.field.draw(buf, p.x + (p.width - w) / 2, by, w, true);
        }
        if matches!(self.phase, Phase::Ready | Phase::Won | Phase::Learned) {
            let b = Rect::new(p.x + p.width / 2 - 7, by, 14, 1);
            fill(buf, b, on(BG, AMBER));
            put(buf, b.x + 2, by, "Enter  play", bold(on(BG, AMBER)));
            self.buttons.push((b, Key::Enter));
        }
        // the trail of questions so far
        let ty = p.bottom().saturating_sub(1 + self.path.len().min(6) as u16);
        let start = self.path.len().saturating_sub(6);
        for (row, depth) in (start..self.path.len()).enumerate() {
            if let Node::Question { text, .. } = self.tree.at(&self.path[..depth]) {
                let ans = if self.path[depth] { "yes" } else { "no" };
                put_max(
                    buf,
                    p.x,
                    ty + row as u16,
                    &format!("{text}? {ans}"),
                    st(DIM),
                    p.width,
                );
            }
        }

        if self.show_tree {
            let t = panel(buf, side, "Everything it knows", CYAN);
            let rows = self.tree.outline();
            let max_scroll = rows.len().saturating_sub(t.height as usize);
            self.scroll = self.scroll.min(max_scroll);
            for (i, (depth, text, leaf, path)) in rows.iter().skip(self.scroll).enumerate() {
                let y = t.y + i as u16;
                if y >= t.bottom() {
                    break;
                }
                let on_path = self.path.starts_with(path)
                    && matches!(self.phase, Phase::Asking | Phase::Guessing);
                let lead = path.last().map_or("", |&a| if a { "y " } else { "n " });
                let x = t.x + (*depth as u16 * 2).min(t.width / 2);
                let colour = if on_path {
                    AMBER
                } else if *leaf {
                    GREEN
                } else {
                    INK
                };
                let x = put(buf, x, y, lead, st(DIM));
                put_max(
                    buf,
                    x,
                    y,
                    text,
                    if *leaf { bold(st(colour)) } else { st(colour) },
                    t.right().saturating_sub(x),
                );
            }
        } else {
            let t = panel(buf, side, "Animals it knows", CYAN);
            let mut names = self.tree.animals();
            names.sort();
            for (i, n) in names.iter().enumerate() {
                let y = t.y + i as u16;
                if y >= t.bottom() {
                    put(
                        buf,
                        t.x,
                        t.bottom() - 1,
                        &format!("and {} more", names.len() - i + 1),
                        st(DIM),
                    );
                    break;
                }
                put_max(buf, t.x, y, n, st(GREEN), t.width);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn learning_and_saving() {
        let mut t = Node::starter();
        // the player thought of an elephant; the computer guessed bird
        t.learn(&[false], "elephant", "Does it have a trunk", true);
        assert_eq!(t.at(&[false, true]), &Node::Animal("elephant".into()));
        assert_eq!(t.at(&[false, false]), &Node::Animal("bird".into()));
        let back = Node::load(&t.save()).unwrap();
        assert_eq!(back, t);
        assert_eq!(back.animals().len(), 3);
    }

    #[test]
    fn tidying() {
        assert_eq!(tidy_question("  is it a mammal? "), "Is it a mammal");
        assert_eq!(tidy_animal("An Owl"), "owl");
        assert_eq!(article("owl"), "an");
    }

    #[test]
    fn bad_files_are_refused() {
        assert!(Node::load("?Only a question\n=one").is_none());
        assert!(Node::load("").is_none());
    }
}
