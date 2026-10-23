//! `gd-ui` — zeron-style design system for desktop-rs.
//!
//! - [`theme`]: tokens (colors, sizes, fonts) — `Theme::of(cx)`.
//! - [`motion`]: the zeron animation catalog (fade-in, menu-in, …).
//! - [`components`]: buttons, checkbox, tabs, toolbar buttons, cards.
//! - [`assets`]: embedded icons + fonts.

pub mod assets;
pub mod components;
pub mod motion;
pub mod overlay;
pub mod theme;
pub mod themes;
pub mod tooltip;

pub use assets::{Assets, Icon};
pub use components::*;
pub use overlay::{ContextMenu, Dialog, MenuEntry, MenuItem};
pub use theme::Theme;
pub use tooltip::tooltip;

use gpui::App;

/// Install fonts and the theme global. Call once at startup.
pub fn init(cx: &mut App) {
    assets::load_fonts(cx);
    cx.set_global(Theme::dark());
}
