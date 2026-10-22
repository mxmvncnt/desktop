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
