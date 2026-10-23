//! The virtualized list around a row renderer, shared by unified and split.

use gd_ui::{Theme, theme::DIFF_ROW_HEIGHT};
use gpui::{AnyElement, App, InteractiveElement, IntoElement, ListState, SharedString, Styled, UniformListScrollHandle, list, px, uniform_list};

/// A `uniform_list` when rows are one line tall, or (`wrap`) a `list` whose
/// rows measure their own height.
pub fn rows_list(
    id: &'static str,
    n: usize,
    wrap: Option<ListState>,
    scroll: &UniformListScrollHandle,
    mono: SharedString,
    render: impl Fn(usize, &Theme, &App) -> AnyElement + 'static,
) -> AnyElement {
    match wrap {
        Some(state) => list(state, move |ix, _w, cx| render(ix, Theme::of(cx), cx))
            .size_full()
            .font_family(mono)
            .text_size(px(12.0))
            .line_height(px(DIFF_ROW_HEIGHT))
            .into_any_element(),
        None => {
            let mut list = uniform_list(id, n, move |range, _w, cx| {
                let theme = Theme::of(cx);
                range.map(|ix| render(ix, theme, cx)).collect()
            })
            .track_scroll(scroll)
            .size_full()
            .font_family(mono)
            .text_size(px(12.0))
            .line_height(px(DIFF_ROW_HEIGHT));
            // Horizontal wheel deltas belong to `Workspace::scroll_diff_x`;
            // without this the list turns them into vertical scrolling.
            list.interactivity().base_style.restrict_scroll_to_axis = Some(true);
            list.into_any_element()
        }
    }
}
