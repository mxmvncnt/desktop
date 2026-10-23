//! Sidebar width: clamping, drag updates and persistence.

use gd_ui::theme::{SIDEBAR_MAX_WIDTH, SIDEBAR_MIN_WIDTH, SIDEBAR_WIDTH};
use gpui::Context;

use crate::workspace::Workspace;

impl Workspace {
    pub fn sidebar_width(&self) -> f32 {
        let w = self.settings.sidebar_width.unwrap_or(SIDEBAR_WIDTH);
        w.clamp(SIDEBAR_MIN_WIDTH, SIDEBAR_MAX_WIDTH)
    }

    /// Live update while dragging; only re-renders when the width changes.
    pub fn drag_sidebar_to(&mut self, x: f32, cx: &mut Context<Self>) {
        let w = x.clamp(SIDEBAR_MIN_WIDTH, SIDEBAR_MAX_WIDTH).round();
        if self.sidebar_width() == w {
            return;
        }
        self.settings.sidebar_width = Some(w);
        self.sidebar_width_dirty = true;
        cx.notify();
    }

    /// Writes the width once the drag ends (mouse up).
    pub fn finish_sidebar_drag(&mut self) {
        if std::mem::take(&mut self.sidebar_width_dirty) {
            self.settings.save();
        }
    }

    pub fn reset_sidebar_width(&mut self, cx: &mut Context<Self>) {
        if self.settings.sidebar_width.take().is_some() {
            self.settings.save();
            cx.notify();
        }
    }
}
