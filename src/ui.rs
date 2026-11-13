//! The collection's shared look: one palette, a title bar and a key bar
//! around every screen, panels, bars, and the number and text fields the
//! games ask their questions with.

use crate::shell::Key;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

pub const BG: Color = Color::Rgb(12, 12, 16);
pub const PAPER: Color = Color::Rgb(22, 22, 28);
pub const LINE: Color = Color::Rgb(70, 68, 62);
pub const INK: Color = Color::Rgb(222, 216, 200);
pub const DIM: Color = Color::Rgb(128, 124, 114);
pub const AMBER: Color = Color::Rgb(255, 176, 0);
pub const GREEN: Color = Color::Rgb(110, 206, 92);
pub const RED: Color = Color::Rgb(232, 86, 70);
pub const CYAN: Color = Color::Rgb(96, 196, 222);
pub const BLUE: Color = Color::Rgb(108, 140, 236);
pub const VIOLET: Color = Color::Rgb(185, 156, 255);

pub fn st(fg: Color) -> Style {
    Style::default().fg(fg).bg(BG)
}

pub fn on(fg: Color, bg: Color) -> Style {
    Style::default().fg(fg).bg(bg)
}

pub fn bold(s: Style) -> Style {
    s.add_modifier(Modifier::BOLD)
}

pub fn mix(a: Color, b: Color, k: f32) -> Color {
    match (a, b) {
        (Color::Rgb(r1, g1, b1), Color::Rgb(r2, g2, b2)) => {
            let m =
                |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * k.clamp(0.0, 1.0)).round() as u8;
            Color::Rgb(m(r1, r2), m(g1, g2), m(b1, b2))
        }
        _ => a,
    }
}

pub fn fill(buf: &mut Buffer, r: Rect, style: Style) {
    let r = r.intersection(buf.area);
    for y in r.top()..r.bottom() {
        for x in r.left()..r.right() {
            buf[(x, y)].set_symbol(" ").set_style(style);
        }
    }
}

/// Text at (x, y), cut at `max` columns and at the buffer's edge. Returns
/// the column after the last character drawn.
pub fn put(buf: &mut Buffer, x: u16, y: u16, s: &str, style: Style) -> u16 {
    put_max(buf, x, y, s, style, u16::MAX)
}

pub fn put_max(buf: &mut Buffer, x: u16, y: u16, s: &str, style: Style, max: u16) -> u16 {
    let a = buf.area;
    if y < a.top() || y >= a.bottom() || x >= a.right() {
        return x;
    }
    let room = (a.right() - x).min(max);
    let (end, _) = buf.set_stringn(x, y, s, room as usize, style);
    end
}

/// Text centred in a row of `r`.
pub fn centre(buf: &mut Buffer, r: Rect, y: u16, s: &str, style: Style) {
    let w = s.chars().count() as u16;
    let x = r.x + r.width.saturating_sub(w) / 2;
    put_max(buf, x, y, s, style, r.width);
}

/// Words wrapped to `width` columns.
pub fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut out = Vec::new();
    for para in text.split('\n') {
        let mut line = String::new();
        for word in para.split_whitespace() {
            if !line.is_empty() && line.chars().count() + 1 + word.chars().count() > width {
                out.push(std::mem::take(&mut line));
            }
            if !line.is_empty() {
                line.push(' ');
            }
            line.push_str(word);
        }
        out.push(line);
    }
    out
}

/// Wrapped text into a rectangle; returns the rows used.
pub fn para(buf: &mut Buffer, r: Rect, text: &str, style: Style) -> u16 {
    let mut y = r.y;
    for line in wrap(text, r.width.max(1) as usize) {
        if y >= r.bottom() {
            break;
        }
        put_max(buf, r.x, y, &line, style, r.width);
        y += 1;
    }
    y - r.y
}

/// The frame every screen sits in: a title bar naming the collection and
/// the game, an optional status at its right, and a bar of keys along the
/// bottom. Returns the space inside.
pub fn frame(
    buf: &mut Buffer,
    area: Rect,
    title: &str,
    status: &str,
    keys: &[(&str, &str)],
) -> Rect {
    fill(buf, area, st(INK));
    if area.height < 3 {
        return area;
    }
    let top = Rect { height: 1, ..area };
    fill(buf, top, on(BG, AMBER));
    let x = put(buf, area.x + 1, area.y, "BCG GAMES ", bold(on(BG, AMBER)));
    let x = put(buf, x, area.y, "▸ ", on(BG, AMBER));
    put(buf, x, area.y, &title.to_uppercase(), bold(on(BG, AMBER)));
    if !status.is_empty() {
        let w = status.chars().count() as u16;
        if area.width > w + 2 {
            put(buf, area.right() - w - 1, area.y, status, on(BG, AMBER));
        }
    }
    let by = area.bottom() - 1;
    fill(
        buf,
        Rect {
            y: by,
            height: 1,
            ..area
        },
        on(DIM, PAPER),
    );
    let mut x = area.x + 1;
    for (k, what) in keys {
        if x + (k.len() + what.len() + 3) as u16 > area.right() {
            break;
        }
        x = put(buf, x, by, k, bold(on(AMBER, PAPER)));
        x = put(buf, x + 1, by, what, on(DIM, PAPER));
        x += 2;
    }
    Rect {
        x: area.x,
        y: area.y + 1,
        width: area.width,
        height: area.height - 2,
    }
}

/// A boxed panel with a title in its top edge; returns the inside.
pub fn panel(buf: &mut Buffer, r: Rect, title: &str, accent: Color) -> Rect {
    if r.width < 4 || r.height < 2 {
        return Rect {
            width: 0,
            height: 0,
            ..r
        };
    }
    let s = st(LINE);
    let (l, rt, t, b) = (r.left(), r.right() - 1, r.top(), r.bottom() - 1);
    for x in l..=rt {
        buf[(x, t)].set_symbol("─").set_style(s);
        buf[(x, b)].set_symbol("─").set_style(s);
    }
    for y in t..=b {
        buf[(l, y)].set_symbol("│").set_style(s);
        buf[(rt, y)].set_symbol("│").set_style(s);
    }
    buf[(l, t)].set_symbol("╭").set_style(s);
    buf[(rt, t)].set_symbol("╮").set_style(s);
    buf[(l, b)].set_symbol("╰").set_style(s);
    buf[(rt, b)].set_symbol("╯").set_style(s);
    if !title.is_empty() {
        put_max(
            buf,
            l + 2,
            t,
            &format!(" {title} "),
            bold(st(accent)),
            r.width.saturating_sub(4),
        );
    }
    Rect {
        x: l + 2,
        y: t + 1,
        width: r.width.saturating_sub(4),
        height: r.height.saturating_sub(2),
    }
}

/// A horizontal bar `w` cells wide, filled to `frac`, in eighths.
pub fn bar(buf: &mut Buffer, x: u16, y: u16, w: u16, frac: f32, fg: Color) {
    const PART: [&str; 8] = [" ", "▏", "▎", "▍", "▌", "▋", "▊", "▉"];
    let eighths = (frac.clamp(0.0, 1.0) * w as f32 * 8.0).round() as u16;
    for i in 0..w {
        let filled = eighths.saturating_sub(i * 8).min(8);
        let sym = if filled == 8 {
            "█"
        } else {
            PART[filled as usize]
        };
        let cx = x + i;
        if cx < buf.area.right() && y < buf.area.bottom() {
            buf[(cx, y)].set_symbol(sym).set_style(on(fg, PAPER));
        }
    }
}

/// When the window is too small for a screen: say so and stop drawing.
pub fn too_small(buf: &mut Buffer, area: Rect, w: u16, h: u16) -> bool {
    if area.width >= w && area.height >= h {
        return false;
    }
    fill(buf, area, st(INK));
    let msg = format!("Make the window at least {w}x{h}");
    let now = format!("(it is {}x{})", area.width, area.height);
    let y = area.y + area.height / 2;
    centre(buf, area, y.saturating_sub(1), &msg, st(AMBER));
    centre(buf, area, y, &now, st(DIM));
    true
}

/// A number the player types or steps with the arrows.
#[derive(Clone, Debug)]
pub struct NumField {
    pub label: &'static str,
    pub text: String,
    pub min: i64,
    pub max: i64,
    pub step: i64,
}

impl NumField {
    pub fn new(label: &'static str, value: i64, min: i64, max: i64, step: i64) -> NumField {
        NumField {
            label,
            text: value.to_string(),
            min,
            max,
            step,
        }
    }

    pub fn value(&self) -> i64 {
        self.text.parse::<i64>().unwrap_or(0)
    }

    pub fn valid(&self) -> bool {
        self.text
            .parse::<i64>()
            .is_ok_and(|v| v >= self.min && v <= self.max)
    }

    pub fn set(&mut self, v: i64) {
        self.text = v.clamp(self.min, self.max).to_string();
    }

    /// Typing, Backspace, and Up/Down (or +/-) to step; true when the key
    /// was used. A digit typed after a step or a fresh focus replaces the
    /// number rather than growing it.
    pub fn key(&mut self, k: &Key, fresh: &mut bool) -> bool {
        match k {
            Key::Char(c) if c.is_ascii_digit() => {
                if *fresh {
                    self.text.clear();
                    *fresh = false;
                }
                if self.text == "0" {
                    self.text.clear();
                }
                if self.text.len() < 9 {
                    self.text.push(*c);
                }
                true
            }
            Key::Char('-') if self.min < 0 && (self.text.is_empty() || *fresh) => {
                self.text = "-".into();
                *fresh = false;
                true
            }
            Key::Backspace => {
                self.text.pop();
                *fresh = false;
                true
            }
            Key::Up | Key::Char('+') | Key::Char('=') => {
                let v = self.value() + self.step;
                self.set(v);
                *fresh = true;
                true
            }
            Key::Down | Key::Char('_') => {
                let v = self.value() - self.step;
                self.set(v);
                *fresh = true;
                true
            }
            Key::PageUp => {
                let v = self.value() + self.step * 10;
                self.set(v);
                *fresh = true;
                true
            }
            Key::PageDown => {
                let v = self.value() - self.step * 10;
                self.set(v);
                *fresh = true;
                true
            }
            _ => false,
        }
    }

    /// `label  [ value ]`, the box lit when focused, red when out of range.
    pub fn draw(&self, buf: &mut Buffer, x: u16, y: u16, label_w: u16, box_w: u16, focus: bool) {
        if label_w > 0 {
            put_max(
                buf,
                x,
                y,
                self.label,
                st(if focus { INK } else { DIM }),
                label_w,
            );
        }
        let bx = x + label_w;
        let fg = if !self.valid() {
            RED
        } else if focus {
            AMBER
        } else {
            INK
        };
        let bg = if focus { Color::Rgb(40, 34, 20) } else { PAPER };
        fill(buf, Rect::new(bx, y, box_w, 1), on(fg, bg));
        let t = if self.text.is_empty() {
            "0"
        } else {
            &self.text
        };
        let w = t.chars().count() as u16;
        put(
            buf,
            bx + box_w.saturating_sub(w + 1),
            y,
            t,
            bold(on(fg, bg)),
        );
        if focus {
            put(buf, bx.saturating_sub(2), y, "▸", st(AMBER));
        }
    }
}

/// A line of text the player types.
#[derive(Clone, Debug, Default)]
pub struct TextField {
    pub text: String,
    pub max: usize,
}

impl TextField {
    pub fn new(max: usize) -> TextField {
        TextField {
            text: String::new(),
            max,
        }
    }

    pub fn key(&mut self, k: &Key) -> bool {
        match k {
            Key::Char(c) if !c.is_control() => {
                if self.text.chars().count() < self.max {
                    self.text.push(*c);
                }
                true
            }
            Key::Backspace => {
                self.text.pop();
                true
            }
            _ => false,
        }
    }

    pub fn draw(&self, buf: &mut Buffer, x: u16, y: u16, w: u16, focus: bool) {
        let bg = if focus { Color::Rgb(40, 34, 20) } else { PAPER };
        fill(buf, Rect::new(x, y, w, 1), on(INK, bg));
        let shown: String = {
            let n = self.text.chars().count();
            let room = w.saturating_sub(2) as usize;
            self.text.chars().skip(n.saturating_sub(room)).collect()
        };
        let end = put(buf, x + 1, y, &shown, bold(on(INK, bg)));
        if focus && end < x + w {
            put(buf, end, y, "▁", on(AMBER, bg));
        }
    }
}

/// A vertical list of choices; returns each row's rectangle, for the mouse.
pub fn menu(buf: &mut Buffer, r: Rect, items: &[String], sel: usize) -> Vec<Rect> {
    let mut rows = Vec::new();
    for (i, it) in items.iter().enumerate() {
        let y = r.y + i as u16;
        if y >= r.bottom() {
            break;
        }
        let row = Rect::new(r.x, y, r.width, 1);
        if i == sel {
            fill(buf, row, on(BG, AMBER));
            put_max(
                buf,
                r.x + 1,
                y,
                &format!("▸ {it}"),
                bold(on(BG, AMBER)),
                r.width.saturating_sub(1),
            );
        } else {
            put_max(
                buf,
                r.x + 1,
                y,
                &format!("  {it}"),
                st(INK),
                r.width.saturating_sub(1),
            );
        }
        rows.push(row);
    }
    rows
}

pub fn hit(r: &Rect, x: u16, y: u16) -> bool {
    x >= r.x && x < r.right() && y >= r.y && y < r.bottom()
}
