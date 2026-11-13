//! What a game keeps between runs, in the player's state folder:
//! $XDG_STATE_HOME/<id> (~/.local/state/<id>), ~/Library/Application
//! Support/<id> on macOS, %LOCALAPPDATA%\<id> on Windows. Plain text files:
//! `key=value` settings and whatever a game writes for itself.

use std::collections::BTreeMap;
use std::path::PathBuf;

pub fn dir(id: &str) -> Option<PathBuf> {
    let env = |k: &str| {
        std::env::var_os(k)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    let base = if cfg!(windows) {
        env("LOCALAPPDATA").or_else(|| env("APPDATA"))
    } else if cfg!(target_os = "macos") {
        env("XDG_STATE_HOME").or_else(|| env("HOME").map(|h| h.join("Library/Application Support")))
    } else {
        env("XDG_STATE_HOME").or_else(|| env("HOME").map(|h| h.join(".local/state")))
    }?;
    Some(base.join(id))
}

pub fn read(id: &str, file: &str) -> Option<String> {
    std::fs::read_to_string(dir(id)?.join(file)).ok()
}

pub fn write(id: &str, file: &str, text: &str) {
    let Some(d) = dir(id) else { return };
    let r = std::fs::create_dir_all(&d).and_then(|_| std::fs::write(d.join(file), text));
    if let Err(e) = r {
        log::warn!("can't save {file} in {}: {e}", d.display());
    }
}

/// `key=value` lines.
#[derive(Default, Debug, Clone)]
pub struct Kv(pub BTreeMap<String, String>);

impl Kv {
    pub fn load(id: &str, file: &str) -> Kv {
        let mut m = BTreeMap::new();
        for line in read(id, file).unwrap_or_default().lines() {
            if let Some((k, v)) = line.split_once('=') {
                m.insert(k.trim().to_string(), v.trim().to_string());
            }
        }
        Kv(m)
    }

    pub fn save(&self, id: &str, file: &str) {
        let text: String = self.0.iter().map(|(k, v)| format!("{k}={v}\n")).collect();
        write(id, file, &text);
    }

    pub fn get<T: std::str::FromStr>(&self, k: &str) -> Option<T> {
        self.0.get(k).and_then(|v| v.parse().ok())
    }

    pub fn set(&mut self, k: &str, v: impl ToString) {
        self.0.insert(k.to_string(), v.to_string());
    }
}
