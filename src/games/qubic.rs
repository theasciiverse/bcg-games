//! 3-D Tic-Tac-Toe: not written yet.

use crate::shell::{Ctx, Game, Key};
use crate::ui::*;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

pub struct Qubic {
    _ctx: Ctx,
    quit: bool,
}

impl Game for Qubic {
    const ID: &'static str = "3d-tic-tac-toe";
    const TITLE: &'static str = "3-D Tic-Tac-Toe";
    const HELP: &'static str = "Not written yet.";

    fn new(ctx: Ctx) -> Self {
        Qubic { _ctx: ctx, quit: false }
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
