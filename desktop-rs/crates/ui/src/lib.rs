//! `gd-ui` — zeron-style design system for the GitHub Desktop rewrite.
//!
//! - [`theme`]: tokens (colors, sizes, fonts) — `Theme::of(cx)`.
//! - [`motion`]: the zeron animation catalog (fade-in, menu-in, …).
//! - [`components`]: buttons, checkbox, tabs, toolbar buttons, cards.
//! - [`assets`]: embedded icons + fonts.

pub mod assets;
pub mod components;
pub mod motion;
pub mod theme;

pub use assets::{Assets, Icon};
pub use components::*;
pub use theme::Theme;

use gpui::App;

/// Install fonts and the theme global. Call once at startup.
pub fn init(cx: &mut App) {
    assets::load_fonts(cx);
    cx.set_global(Theme::dark());
}
