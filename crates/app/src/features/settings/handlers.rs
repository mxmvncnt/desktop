//! Workspace-side settings handlers: open/close the modal, switch page,
//! apply the theme.

use gd_ui::Theme;
use gpui::{Context, Window};

use super::{SettingsPage, ThemeMode};
use crate::workspace::Workspace;

impl Workspace {
    pub fn toggle_settings(&mut self, cx: &mut Context<Self>) {
        self.settings_open = !self.settings_open;
        cx.notify();
    }

    pub(super) fn set_settings_page(&mut self, page: SettingsPage, cx: &mut Context<Self>) {
        self.settings_page = page;
        cx.notify();
    }

    /// Resolve the theme setting (System follows the window appearance) and install it.
    pub fn apply_theme(&mut self, window: &Window, cx: &mut Context<Self>) {
        use gpui::WindowAppearance as A;
        let system_light = matches!(window.appearance(), A::Light | A::VibrantLight);
        let light = self.settings.theme.is_light(system_light);
        cx.set_global(Theme::for_mode(light));
        cx.notify();
    }

    pub(super) fn set_theme_mode(&mut self, mode: ThemeMode, window: &Window, cx: &mut Context<Self>) {
        self.settings.theme = mode;
        self.settings.save();
        self.apply_theme(window, cx);
    }
}
