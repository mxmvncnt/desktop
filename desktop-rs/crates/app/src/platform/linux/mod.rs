//! Linux (Wayland + X11): client-side decorations, XDG config dir.

mod chrome;
pub use chrome::*;

use std::path::PathBuf;

use gpui::WindowOptions;

/// Ctrl is the "secondary" modifier (macOS uses cmd).
pub const SECONDARY: &str = "ctrl";
/// How the secondary modifier is written in UI text.
pub const SECONDARY_LABEL: &str = "Ctrl";
pub const REFRESH_KEY: &str = "f5";
pub const QUIT_KEY: &str = "ctrl-q";

/// `$XDG_CONFIG_HOME`, else `~/.config`.
pub fn config_base_dir() -> Option<PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
}

pub fn configure_window(options: &mut WindowOptions) {
    // Draw our own frame (see chrome.rs). GNOME never provides server
    // decorations; KDE/X11 WMs may still negotiate SSD, in which case
    // chrome.rs draws nothing.
    options.window_decorations = Some(gpui::WindowDecorations::Client);
    // Transparent so the rounded corners + shadow margin show the desktop
    // behind the window.
    options.linux_window_background = gpui::LinuxWindowBackground::Transparent;
}
