//! Persisted app state (the repository list + last selection).
//!
//! Stored as plain text in `$GD_CONFIG_DIR` (override, used by dev/test runs so
//! nothing is written outside the project), else
//! `$XDG_CONFIG_HOME/github-desktop-rs`, else `~/.config/github-desktop-rs`.
//!
//! Format of `repositories`: one absolute path per line; a line starting with
//! `*` marks the last-selected repository.

use std::path::PathBuf;

pub fn config_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("GD_CONFIG_DIR") {
        return Some(PathBuf::from(dir));
    }
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("github-desktop-rs"))
}

#[derive(Default, Debug, Clone)]
pub struct Settings {
    pub repositories: Vec<PathBuf>,
    pub selected: Option<usize>,
}

impl Settings {
    pub fn load() -> Self {
        let Some(file) = config_dir().map(|d| d.join("repositories")) else {
            return Self::default();
        };
        let Ok(text) = std::fs::read_to_string(file) else {
            return Self::default();
        };
        let mut s = Self::default();
        for line in text.lines().filter(|l| !l.trim().is_empty()) {
            let (sel, path) = match line.strip_prefix('*') {
                Some(p) => (true, p),
                None => (false, line),
            };
            if sel {
                s.selected = Some(s.repositories.len());
            }
            s.repositories.push(PathBuf::from(path));
        }
        s
    }

    pub fn save(&self) {
        let Some(dir) = config_dir() else { return };
        if let Err(e) = std::fs::create_dir_all(&dir) {
            log::error!("cannot create config dir: {e}");
            return;
        }
        let text: String = self
            .repositories
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let mark = if Some(i) == self.selected { "*" } else { "" };
                format!("{mark}{}\n", p.display())
            })
            .collect();
        if let Err(e) = std::fs::write(dir.join("repositories"), text) {
            log::error!("cannot save settings: {e}");
        }
    }
}
