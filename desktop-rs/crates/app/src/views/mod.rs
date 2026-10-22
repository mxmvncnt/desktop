//! View fragments, implemented as `impl Workspace` blocks so they share the
//! root entity's state. Heavy lists are always `uniform_list` (virtualized).

mod changes;
pub mod diff;
pub mod diff_options;
mod diff_split;
mod history;
pub mod history_menu;
mod in_progress_banner;
pub mod menus;
pub mod merge;
mod pickers;
pub mod settings;
pub mod stashes;
mod welcome;

use gd_git::FileStatus;
use gd_ui::Theme;
use gpui::{App, Hsla};

/// Color for a file-status badge.
pub fn status_color(status: FileStatus, cx: &App) -> Hsla {
    let t = Theme::of(cx);
    match status {
        FileStatus::New | FileStatus::Untracked => t.success,
        FileStatus::Deleted => t.danger,
        FileStatus::Conflicted => t.danger,
        FileStatus::Renamed | FileStatus::Copied => t.accent,
        FileStatus::Modified | FileStatus::TypeChanged => t.warning,
    }
}

/// Split `a/b/c.rs` into (`a/b/`, `c.rs`) for the dim-dir / bright-name rows.
pub fn split_path(path: &str) -> (&str, &str) {
    match path.rfind('/') {
        Some(i) => (&path[..=i], &path[i + 1..]),
        None => ("", path),
    }
}

/// "3 minutes ago"-style relative time from unix seconds.
pub fn relative_time(unix: i64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let d = (now - unix).max(0);
    let (n, unit) = match d {
        0..=59 => return "just now".into(),
        60..=3599 => (d / 60, "minute"),
        3600..=86_399 => (d / 3600, "hour"),
        86_400..=2_591_999 => (d / 86_400, "day"),
        2_592_000..=31_535_999 => (d / 2_592_000, "month"),
        _ => (d / 31_536_000, "year"),
    };
    format!("{n} {unit}{} ago", if n == 1 { "" } else { "s" })
}
