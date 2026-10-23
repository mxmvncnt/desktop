//! Linux (Wayland + X11): client-side decorations.

mod chrome;
pub use chrome::*;

use gpui::WindowOptions;

/// Ctrl is the "secondary" modifier (macOS uses cmd).
pub const SECONDARY: &str = "ctrl";
/// How the secondary modifier is written in UI text.
pub const SECONDARY_LABEL: &str = "Ctrl";
pub const REFRESH_KEY: &str = "f5";
pub const QUIT_KEY: &str = "ctrl-q";

pub fn configure_window(options: &mut WindowOptions) {
    // Draw our own frame (see chrome.rs). GNOME never provides server
    // decorations; KDE/X11 WMs may still negotiate SSD, in which case
    // chrome.rs draws nothing.
    options.window_decorations = Some(gpui::WindowDecorations::Client);
    // Transparent so the rounded corners + shadow margin show the desktop
    // behind the window.
    options.linux_window_background = gpui::LinuxWindowBackground::Transparent;
}
