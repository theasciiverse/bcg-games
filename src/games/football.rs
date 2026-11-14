//! Football: not written yet.

use crate::shell::{Ctx, Game, Key};
use crate::ui::*;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

pub struct Football {
    _ctx: Ctx,
    quit: bool,
}

impl Game for Football {
    const ID: &'static str = "football";
    const TITLE: &'static str = "Football";
    const HELP: &'static str = "Not written yet.";

    fn new(ctx: Ctx) -> Self {
        Football { _ctx: ctx, quit: false }
    }

    fn draw(&mut self, area: Rect, buf: &mut Buffer) {
        frame(buf, area, Self::TITLE, "", &[("Esc", "quit")]);
    }

    fn key(&mut self, k: Key) {
        if matches!(k, Key::Esc | Key::Char('q')) {
            self.quit = true;
        }
    }

    fn quit(&self) -> bool {
        self.quit
    }

    fn snapshot(&self) -> String {
        String::new()
    }
}
