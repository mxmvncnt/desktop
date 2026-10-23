//! The virtualized list of a unified diff.

use std::sync::Arc;

use gd_git::Diff;
use gpui::{App, IntoElement, ListState, Pixels, RenderOnce, SharedString, UniformListScrollHandle, Window};

use super::expander::Expander;
use super::highlight::DiffHighlight;
use super::row_select::RowSelect;
use super::rows_list::rows_list;
use super::text_flow::TextFlow;
use super::text_select::TextSelect;
use super::unified_row::UnifiedRows;

#[derive(IntoElement)]
pub struct UnifiedList {
    pub diff: Arc<Diff>,
    pub hl: Option<Arc<DiffHighlight>>,
    pub select: Option<RowSelect>,
    pub exp: Option<Expander>,
    /// Row count including the trailing "expand down" row.
    pub rows: usize,
    pub num_w: Pixels,
    pub scroll: UniformListScrollHandle,
    pub wrap: Option<ListState>,
    pub flow: TextFlow,
    pub tsel: TextSelect,
    pub mono: SharedString,
}

impl RenderOnce for UnifiedList {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let Self { diff, hl, select, exp, rows, num_w, scroll, wrap, flow, tsel, mono } = self;
        rows_list("diff-lines", rows, wrap, &scroll, mono, move |ix, theme, cx| {
            let sel = select.as_ref().map(|s| (s, s.repo.read(cx)));
            UnifiedRows { diff: &diff, hl: hl.as_deref(), sel, exp: exp.as_ref(), num_w, flow, tsel: &tsel, theme }.row(ix)
        })
    }
}
