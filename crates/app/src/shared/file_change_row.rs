//! One changed file of a commit or stash: directory, name and status badge.

use gd_git::FileChange;
use gd_ui::{Theme, pressable};
use gpui::{App, IntoElement, RenderOnce, Window, div, prelude::*, px};

use super::file_status_color::status_color;
use super::split_path::split_path;

#[derive(IntoElement)]
pub struct FileChangeRow {
    ix: usize,
    change: FileChange,
    selected: bool,
    /// Element id namespace, so lists on screen together never collide.
    id: &'static str,
    on_select: Box<dyn Fn(&mut Window, &mut App)>,
}

impl FileChangeRow {
    pub fn new(
        id: &'static str,
        ix: usize,
        change: FileChange,
        selected: bool,
        on_select: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        Self { ix, change, selected, id, on_select: Box::new(on_select) }
    }
}

impl RenderOnce for FileChangeRow {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::of(cx).clone();
        let (ix, on_select) = (self.ix, self.on_select);
        let f = &self.change;
        let (dir, name) = split_path(&f.path);
        pressable((self.id, ix), self.selected, cx)
            .flex()
            .items_center()
            .gap(px(8.0))
            .h(px(28.0))
            .w_full()
            .px(px(8.0))
            .rounded(px(6.0))
            .text_size(px(12.5))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .child(div().flex_shrink(1.0).min_w_0().truncate().text_color(theme.text_faint).child(dir.to_string()))
                    .child(
                        div()
                            .flex_none()
                            .text_color(if self.selected { theme.text } else { theme.text_muted })
                            .child(name.to_string()),
                    ),
            )
            .child(
                div()
                    .flex_none()
                    .w(px(14.0))
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(status_color(f.status, cx))
                    .child(f.status.badge()),
            )
            .on_click(move |_, w, cx| on_select(w, cx))
    }
}
