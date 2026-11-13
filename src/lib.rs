//! bcg-games: eight games after David H. Ahl's *BASIC Computer Games*
//! (1978), each rebuilt from a written description of its rules and our own
//! play of the original, never from its listing (CLEANROOM.md). One crate,
//! eight binaries; this library holds the shared shell, look and sound, and
//! each game's rules and screens under `games`.

pub mod audio;
pub mod games;
pub mod rng;
pub mod shell;
pub mod store;
pub mod ui;
