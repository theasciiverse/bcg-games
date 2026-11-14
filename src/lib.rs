//! bcg-games: games after David H. Ahl's *BASIC Computer Games*
//! (1978), each rebuilt from rules written in our own words, never from its
//! listing (CLEANROOM.md). One crate, one binary per game; this library
//! holds the shared shell, look and sound, and each game's rules and
//! screens under `games`.

pub mod audio;
pub mod games;
pub mod rng;
pub mod shell;
pub mod store;
pub mod ui;
