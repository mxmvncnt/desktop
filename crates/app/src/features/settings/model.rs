//! Persisted app state (the repository list + last selection).
//!
//! Stored as plain text in `$GD_CONFIG_DIR` (override, used by dev/test runs so
//! nothing is written outside the project), else
//! `desktop-rs` under the platform config dir (`dirs::config_dir`:
//! `$XDG_CONFIG_HOME` or `~/.config` on Linux, `~/Library/Application Support` on macOS).
//!
//! Format of `repositories`: one absolute path per line; a line starting with
//! `*` marks the last-selected repository.
//!
//! Display preferences live in a second file, `display` (`key=value` lines,
//! unknown keys ignored), so the `repositories` format is untouched and files
//! written by older builds keep loading: `diff_split=true|false`,
//! `diff_ignore_whitespace=true|false`, `diff_wrap=true|false`,
//! `theme=system|light|dark`, `color_theme=<id>` (see `gd_ui::themes`), `sidebar_width=<px>`.

use std::path::PathBuf;

pub fn config_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("GD_CONFIG_DIR") {
        return Some(PathBuf::from(dir));
    }
    Some(dirs::config_dir()?.join("desktop-rs"))
}

/// Which theme to use; `System` follows the OS appearance.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeMode {
    #[default]
    System,
    Light,
    Dark,
}

impl std::fmt::Display for ThemeMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::System => "system",
            Self::Light => "light",
            Self::Dark => "dark",
        })
    }
}

impl std::str::FromStr for ThemeMode {
    type Err = ();

    fn from_str(v: &str) -> Result<Self, ()> {
        Self::ALL.into_iter().find(|m| m.to_string() == v).ok_or(())
    }
}

impl ThemeMode {
    pub const ALL: [ThemeMode; 3] = [Self::System, Self::Light, Self::Dark];

    pub fn label(self) -> &'static str {
        match self {
            Self::System => "System",
            Self::Light => "Light",
            Self::Dark => "Dark",
        }
    }

    /// Whether the light theme applies, given the OS appearance.
    pub fn is_light(self, system_light: bool) -> bool {
        match self {
            Self::System => system_light,
            Self::Light => true,
            Self::Dark => false,
        }
    }
}

#[derive(Default, Debug, Clone)]
pub struct Settings {
    pub repositories: Vec<PathBuf>,
    pub selected: Option<usize>,
    /// Side-by-side diff instead of unified (app-wide, like Desktop).
    pub diff_split: bool,
    /// Hide whitespace changes in diffs (`git diff -w`).
    pub diff_ignore_whitespace: bool,
    /// Wrap long diff lines instead of scrolling horizontally.
    pub diff_wrap: bool,
    pub theme: ThemeMode,
    /// Sidebar width in px; `None` = default (`gd_ui::theme::SIDEBAR_WIDTH`).
    pub sidebar_width: Option<f32>,
    /// Color theme family id (`gd_ui::themes::COLOR_THEMES`).
    pub color_theme: String,
}

impl Settings {
    pub fn load() -> Self {
        let mut s = Self::default();
        let Some(dir) = config_dir() else { return s };
        if let Ok(text) = std::fs::read_to_string(dir.join("display")) {
            s.apply_display(&text);
        }
        if let Ok(text) = std::fs::read_to_string(dir.join("repositories")) {
            s.apply_repositories(&text);
        }
        s
    }

    /// `display` file: `key=value` lines, unknown keys and bad values ignored
    /// (booleans: anything but `true` is false).
    fn apply_display(&mut self, text: &str) {
        for (key, v) in text.lines().filter_map(|l| l.split_once('=')) {
            let flag = v.trim() == "true";
            match key.trim() {
                "color_theme" => {
                    self.color_theme = gd_ui::themes::find(v.trim()).map_or_else(String::new, |t| t.id.into())
                }
                "theme" => self.theme = v.trim().parse().unwrap_or_default(),
                "diff_split" => self.diff_split = flag,
                "diff_ignore_whitespace" => self.diff_ignore_whitespace = flag,
                "diff_wrap" => self.diff_wrap = flag,
                "sidebar_width" => self.sidebar_width = v.trim().parse().ok(),
                _ => {}
            }
        }
    }

    /// `repositories` file: one path per line, `*` prefix marks the selection.
    fn apply_repositories(&mut self, text: &str) {
        for line in text.lines().filter(|l| !l.trim().is_empty()) {
            let (selected, path) = match line.strip_prefix('*') {
                Some(p) => (true, p),
                None => (false, line),
            };
            if selected {
                self.selected = Some(self.repositories.len());
            }
            self.repositories.push(PathBuf::from(path));
        }
    }

    /// Id of the active color theme (`zeron` when unset or unknown).
    pub fn color_theme_id(&self) -> &str {
        gd_ui::themes::find(&self.color_theme).map_or(gd_ui::themes::DEFAULT_THEME, |t| t.id)
    }

    fn display_text(&self) -> String {
        format!(
            "diff_split={}\ndiff_ignore_whitespace={}\ndiff_wrap={}\ntheme={}\ncolor_theme={}\n{}",
            self.diff_split,
            self.diff_ignore_whitespace,
            self.diff_wrap,
            self.theme,
            self.color_theme_id(),
            self.sidebar_width.map(|w| format!("sidebar_width={w}\n")).unwrap_or_default()
        )
    }

    fn repositories_text(&self) -> String {
        let marked = |i| if Some(i) == self.selected { "*" } else { "" };
        (self.repositories.iter().enumerate())
            .map(|(i, p)| format!("{}{}\n", marked(i), p.display()))
            .collect()
    }

    pub fn save(&self) {
        let Some(dir) = config_dir() else { return };
        if let Err(e) = std::fs::create_dir_all(&dir) {
            log::error!("cannot create config dir: {e}");
            return;
        }
        for (name, text) in [
            ("repositories", self.repositories_text()),
            ("display", self.display_text()),
        ] {
            if let Err(e) = std::fs::write(dir.join(name), text) {
                log::error!("cannot save {name}: {e}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_old_files_and_round_trips() {
        let mut s = Settings::default();
        s.apply_display("diff_split=true\nfuture=1\ntheme=dark\n");
        s.apply_repositories("/a\n*/b c\n\n");
        assert!(s.diff_split && !s.diff_wrap);
        assert_eq!(s.theme, ThemeMode::Dark);
        assert_eq!(s.selected, Some(1));
        assert_eq!(s.repositories_text(), "/a\n*/b c\n");
        let mut t = Settings::default();
        t.apply_display(&s.display_text());
        assert_eq!((t.diff_split, t.theme), (true, ThemeMode::Dark));
        let mut u = Settings::default();
        u.apply_display("theme=bogus\ncolor_theme=nord");
        assert_eq!(u.theme, ThemeMode::System);
        assert_eq!(u.sidebar_width, None);
        u.apply_display("sidebar_width=345\n");
        assert_eq!(u.sidebar_width, Some(345.0));
        assert!(u.display_text().contains("sidebar_width=345"));
        assert_eq!(u.color_theme_id(), "nord");
        u.apply_display("color_theme=bogus");
        assert_eq!(u.color_theme_id(), "zeron");
    }
}
