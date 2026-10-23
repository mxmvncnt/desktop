//! Color for a file-status badge.

use gd_git::FileStatus;
use gd_ui::Theme;
use gpui::{App, Hsla};

pub fn status_color(status: FileStatus, cx: &App) -> Hsla {
    let t = Theme::of(cx);
    match status {
        FileStatus::New | FileStatus::Untracked => t.success,
        FileStatus::Deleted | FileStatus::Conflicted => t.danger,
        FileStatus::Renamed | FileStatus::Copied => t.accent,
        FileStatus::Modified | FileStatus::TypeChanged => t.warning,
    }
}
