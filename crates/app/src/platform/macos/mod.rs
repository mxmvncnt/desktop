//! macOS: native window frame with a transparent titlebar, so the toolbar
//! doubles as the titlebar and the traffic lights sit inside it.

mod chrome;
pub use chrome::*;

use gd_ui::theme::TOOLBAR_HEIGHT;
use gpui::{WindowOptions, point, px};

/// Cmd is the "secondary" modifier.
pub const SECONDARY: &str = "cmd";
/// How the secondary modifier is written in UI text.
pub const SECONDARY_LABEL: &str = "Cmd";
/// GitHub Desktop's Refresh is Cmd+R on macOS.
pub const REFRESH_KEY: &str = "cmd-r";
pub const QUIT_KEY: &str = "cmd-q";

pub fn configure_window(options: &mut WindowOptions) {
    let titlebar = options.titlebar.get_or_insert_with(Default::default);
    titlebar.appears_transparent = true;
    titlebar.traffic_light_position = Some(point(px(14.0), px((TOOLBAR_HEIGHT - 12.0) / 2.0)));
}
