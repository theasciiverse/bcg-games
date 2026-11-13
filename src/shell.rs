//! What every game shares around its rules: the command line, the
//! terminal, keys, mouse and gamepad turned into one small vocabulary, the
//! frame clock, and session logging (RUST_LOG=debug <game> 2>log: every key
//! and click as it arrives, and a snapshot of the game once a second).

use crate::audio::Audio;
use crate::rng::Rng;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers,
    MouseButton, MouseEventKind,
};
use ratatui::crossterm::execute;
use ratatui::layout::Rect;
use std::io;
use std::time::{Duration, Instant, SystemTime};

const FRAME: Duration = Duration::from_millis(16);
const SNAPSHOT_EVERY: Duration = Duration::from_secs(1);

#[derive(Clone, Debug, PartialEq)]
pub enum Key {
    Char(char),
    Up,
    Down,
    Left,
    Right,
    Enter,
    Esc,
    Backspace,
    Tab,
    BackTab,
    PageUp,
    PageDown,
    Home,
    End,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MouseKind {
    Click,
    Right,
    Move,
    ScrollUp,
    ScrollDown,
}

#[derive(Clone, Copy, Debug)]
pub struct Mouse {
    pub x: u16,
    pub y: u16,
    pub kind: MouseKind,
}

pub struct Ctx {
    pub audio: Audio,
    pub rng: Rng,
    /// the game plays itself, for the catalog preview
    pub demo: bool,
}

pub trait Game: Sized {
    /// the binary's and the state folder's name
    const ID: &'static str;
    const TITLE: &'static str;
    /// what the game is and how to play it, for --help
    const HELP: &'static str;

    fn new(ctx: Ctx) -> Self;
    fn draw(&mut self, area: Rect, buf: &mut Buffer);
    fn key(&mut self, k: Key);
    fn mouse(&mut self, _m: Mouse) {}
    fn tick(&mut self, _dt: f32) {}
    fn quit(&self) -> bool;
    fn snapshot(&self) -> String;
}

pub fn main<G: Game>() -> io::Result<()> {
    env_logger::init();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let has = |f: &str| args.iter().any(|a| a == f);
    let after = |f: &str| {
        args.iter()
            .position(|a| a == f)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    if has("--help") || has("-h") {
        print!(
            "{} [--demo] [--mute] [--seed N]\n\n{}\n\n  --demo     the game plays itself\n  --mute     no sound (M turns it on and off while playing)\n  --seed N   a fixed random seed\n\nPart of bcg-games, after David H. Ahl's BASIC Computer Games (1978).\nSaved in the state folder ({}).\nLogging: RUST_LOG=debug {} 2>{}.log\n",
            G::ID,
            G::HELP.trim_end(),
            crate::store::dir(G::ID).map(|d| d.display().to_string()).unwrap_or_default(),
            G::ID,
            G::ID
        );
        return Ok(());
    }
    if has("--version") || has("-V") {
        println!("{} {}", G::ID, env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    let seed = after("--seed")
        .and_then(|v| v.parse().ok())
        .unwrap_or_else(|| {
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .map_or(1, |d| d.as_nanos() as u64)
        });
    let settings = crate::store::Kv::load(G::ID, "settings");
    let muted = has("--mute") || settings.get::<bool>("muted").unwrap_or(false);
    // open audio before the alternate screen, so any ALSA chatter lands above it
    let audio = Audio::new(muted);
    log::info!(
        "start {} {} audio {} muted {muted} seed {seed}",
        G::ID,
        env!("CARGO_PKG_VERSION"),
        audio.available()
    );
    let mut game = G::new(Ctx {
        audio,
        rng: Rng::new(seed),
        demo: has("--demo"),
    });

    let mut terminal = ratatui::init();
    execute!(io::stdout(), EnableMouseCapture)?;
    let mut pads = Pads::new();
    let result = run(&mut terminal, &mut game, &mut pads);
    let _ = execute!(io::stdout(), DisableMouseCapture);
    ratatui::restore();
    result
}

/// Games read M for the sound through this, so each one keeps the choice.
pub fn remember_mute(id: &str, muted: bool) {
    let mut s = crate::store::Kv::load(id, "settings");
    s.set("muted", muted);
    s.save(id, "settings");
}

fn run<G: Game>(
    terminal: &mut ratatui::DefaultTerminal,
    game: &mut G,
    pads: &mut Pads,
) -> io::Result<()> {
    let mut last = Instant::now();
    let mut snapshot = Instant::now();
    loop {
        terminal.draw(|f| game.draw(f.area(), f.buffer_mut()))?;
        let deadline = Instant::now() + FRAME;
        while let Some(wait) = deadline.checked_duration_since(Instant::now()) {
            if !event::poll(wait)? {
                break;
            }
            match event::read()? {
                Event::Key(k) if k.kind != KeyEventKind::Release => {
                    log::debug!("key {:?} {:?}", k.code, k.modifiers);
                    if k.modifiers.contains(KeyModifiers::CONTROL)
                        && matches!(k.code, KeyCode::Char('c'))
                    {
                        log::info!("quit by ctrl-c");
                        return Ok(());
                    }
                    let key = match k.code {
                        KeyCode::Char(c) => Some(Key::Char(c)),
                        KeyCode::Up => Some(Key::Up),
                        KeyCode::Down => Some(Key::Down),
                        KeyCode::Left => Some(Key::Left),
                        KeyCode::Right => Some(Key::Right),
                        KeyCode::Enter => Some(Key::Enter),
                        KeyCode::Esc => Some(Key::Esc),
                        KeyCode::Backspace => Some(Key::Backspace),
                        KeyCode::Tab => Some(Key::Tab),
                        KeyCode::BackTab => Some(Key::BackTab),
                        KeyCode::PageUp => Some(Key::PageUp),
                        KeyCode::PageDown => Some(Key::PageDown),
                        KeyCode::Home => Some(Key::Home),
                        KeyCode::End => Some(Key::End),
                        _ => None,
                    };
                    if let Some(key) = key {
                        game.key(key);
                    }
                }
                Event::Mouse(m) => {
                    let kind = match m.kind {
                        MouseEventKind::Down(MouseButton::Left) => Some(MouseKind::Click),
                        MouseEventKind::Down(MouseButton::Right) => Some(MouseKind::Right),
                        MouseEventKind::Moved | MouseEventKind::Drag(_) => Some(MouseKind::Move),
                        MouseEventKind::ScrollUp => Some(MouseKind::ScrollUp),
                        MouseEventKind::ScrollDown => Some(MouseKind::ScrollDown),
                        _ => None,
                    };
                    if let Some(kind) = kind {
                        if kind == MouseKind::Move {
                            log::trace!("mouse at {},{}", m.column, m.row);
                        } else {
                            log::debug!("mouse {kind:?} {},{}", m.column, m.row);
                        }
                        game.mouse(Mouse {
                            x: m.column,
                            y: m.row,
                            kind,
                        });
                    }
                }
                Event::Resize(w, h) => log::info!("resize {w}x{h}"),
                _ => {}
            }
            if game.quit() {
                return Ok(());
            }
        }
        for k in pads.poll() {
            game.key(k);
        }
        if game.quit() {
            return Ok(());
        }
        let now = Instant::now();
        let dt = now.duration_since(last).as_secs_f32().min(0.1);
        last = now;
        game.tick(dt);
        if snapshot.elapsed() >= SNAPSHOT_EVERY {
            snapshot = Instant::now();
            log::debug!("snapshot {}", game.snapshot());
        }
    }
}

/// Gamepads speak the same keys: the d-pad is the arrows, A is Enter,
/// B is Esc, X is Backspace, Y is Tab, and the shoulders page.
struct Pads {
    #[cfg(feature = "gamepad")]
    gilrs: Option<gilrs::Gilrs>,
}

impl Pads {
    fn new() -> Pads {
        #[cfg(feature = "gamepad")]
        {
            let gilrs = match gilrs::Gilrs::new() {
                Ok(g) => Some(g),
                Err(e) => {
                    log::warn!("no gamepads: {e}");
                    None
                }
            };
            Pads { gilrs }
        }
        #[cfg(not(feature = "gamepad"))]
        Pads {}
    }

    #[cfg(not(feature = "gamepad"))]
    fn poll(&mut self) -> Vec<Key> {
        Vec::new()
    }

    #[cfg(feature = "gamepad")]
    fn poll(&mut self) -> Vec<Key> {
        use gilrs::{Button, EventType};
        let mut out = Vec::new();
        let Some(g) = &mut self.gilrs else { return out };
        while let Some(ev) = g.next_event() {
            match ev.event {
                EventType::Connected => {
                    log::info!("gamepad connected: {}", g.gamepad(ev.id).name())
                }
                EventType::ButtonPressed(b, _) => {
                    // gilrs also reports 3D mice and tablet pads as gamepads;
                    // only trust the ones SDL knows
                    let any = std::env::var("BCG_ANY_PAD").is_ok_and(|v| v == "1");
                    if !any
                        && g.gamepad(ev.id).mapping_source() != gilrs::MappingSource::SdlMappings
                    {
                        continue;
                    }
                    log::debug!("pad button {b:?}");
                    let k = match b {
                        Button::DPadUp => Some(Key::Up),
                        Button::DPadDown => Some(Key::Down),
                        Button::DPadLeft => Some(Key::Left),
                        Button::DPadRight => Some(Key::Right),
                        Button::South => Some(Key::Enter),
                        Button::East | Button::Select => Some(Key::Esc),
                        Button::West => Some(Key::Backspace),
                        Button::North => Some(Key::Tab),
                        Button::LeftTrigger => Some(Key::PageDown),
                        Button::RightTrigger => Some(Key::PageUp),
                        Button::Start => Some(Key::Enter),
                        _ => None,
                    };
                    out.extend(k);
                }
                _ => {}
            }
        }
        out
    }
}
