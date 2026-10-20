//! Persisted app state (the repository list + last selection).
//!
//! Stored as plain text in `$GD_CONFIG_DIR` (override, used by dev/test runs so
//! nothing is written outside the project), else
//! `$XDG_CONFIG_HOME/github-desktop-rs`, else `~/.config/github-desktop-rs`.
//!
//! Format of `repositories`: one absolute path per line; a line starting with
//! `*` marks the last-selected repository.
//!
//! Display preferences live in a second file, `display` (`key=value` lines,
//! unknown keys ignored), so the `repositories` format is untouched and files
//! written by older builds keep loading: `diff_split=true|false`,
//! `diff_ignore_whitespace=true|false`.

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
    /// Side-by-side diff instead of unified (app-wide, like Desktop).
    pub diff_split: bool,
    /// Hide whitespace changes in diffs (`git diff -w`).
    pub diff_ignore_whitespace: bool,
}

impl Settings {
    pub fn load() -> Self {
        let mut s = Self::default();
        let Some(dir) = config_dir() else { return s };
        if let Ok(text) = std::fs::read_to_string(dir.join("display")) {
            for line in text.lines() {
                match line.split_once('=').map(|(k, v)| (k.trim(), v.trim() == "true")) {
                    Some(("diff_split", v)) => s.diff_split = v,
                    Some(("diff_ignore_whitespace", v)) => s.diff_ignore_whitespace = v,
                    _ => {}
                }
            }
        }
        let Ok(text) = std::fs::read_to_string(dir.join("repositories")) else {
            return s;
        };
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
        let display = format!(
            "diff_split={}\ndiff_ignore_whitespace={}\n",
            self.diff_split, self.diff_ignore_whitespace
        );
        if let Err(e) = std::fs::write(dir.join("display"), display) {
            log::error!("cannot save display settings: {e}");
        }
    }
}
