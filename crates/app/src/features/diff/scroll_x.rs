//! Horizontal scrolling of the (non-wrapped) diff text.

use gd_ui::Theme;
use gpui::{Context, Pixels, TextRun, Window, px};

use crate::workspace::Workspace;

impl Workspace {
    /// Wheel/trackpad horizontal scroll of the (non-wrapped) diff, shared by
    /// both split columns. Clamped so the widest line can just be read to its end.
    pub fn scroll_diff_x(&mut self, by: Pixels, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ext) = &self.diff_view.extent else { return };
        let Some(line) = ext.diff.lines.get(ext.diff.longest_line).filter(|l| !l.text.is_empty()) else {
            return;
        };
        let run = TextRun {
            len: line.text.len(),
            font: gpui::font(Theme::of(cx).font_mono.clone()),
            color: gpui::black(),
            background_color: None,
            underline: None,
            strikethrough: None,
            letter_spacing: None,
        };
        let width = window.text_system().shape_line(line.text.clone().into(), px(12.0), &[run]).width;
        let view = self.diff_scroll.0.borrow().base_handle.bounds().size.width;
        let visible = (view - ext.fixed) / ext.columns;
        let max = (width + px(24.0) - visible).max(px(0.0));
        let next = (self.diff_view.hscroll + by).min(max).max(px(0.0));
        if next != self.diff_view.hscroll {
            self.diff_view.hscroll = next;
            cx.notify();
        }
    }
}
