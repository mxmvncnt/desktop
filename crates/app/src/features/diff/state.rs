//! State of the diff viewer: the `Workspace::diff_view` field (options popover,
//! split-row cache, horizontal scroll, wrap list) and the gutter pointer state.

use std::sync::Arc;

use gd_git::Diff;
use gpui::{ListAlignment, ListState, Pixels, Point, px};

use super::split_diff::SplitDiff;

/// Workspace-side state for the diff viewer (one field on `Workspace`).
pub struct DiffView {
    /// Open options popover position (window coordinates).
    pub menu: Option<Point<Pixels>>,
    pub menu_gen: usize,
    /// Paired rows of the diff currently shown in split mode.
    pub(super) split_cache: Option<Arc<SplitDiff>>,
    /// Horizontal scroll of the diff text (both split columns), see `TextFlow`.
    pub hscroll: Pixels,
    /// What `Workspace::scroll_diff_x` needs to clamp `hscroll`.
    pub extent: Option<HExtent>,
    /// Variable-height list used while lines wrap, and the diff (and split
    /// mode) it was last reset for.
    wrap_list: ListState,
    pub(super) wrap_for: Option<(Arc<Diff>, bool)>,
}

impl Default for DiffView {
    fn default() -> Self {
        Self {
            menu: None,
            menu_gen: 0,
            split_cache: None,
            hscroll: px(0.0),
            extent: None,
            wrap_list: ListState::new(0, ListAlignment::Top, px(400.0)),
            wrap_for: None,
        }
    }
}

/// Horizontal layout of the shown diff: the text columns share the list
/// width minus `fixed` (gutters, markers) evenly.
pub struct HExtent {
    pub diff: Arc<Diff>,
    pub fixed: Pixels,
    pub columns: f32,
}

impl DiffView {
    pub fn set_extent(&mut self, diff: &Arc<Diff>, fixed: Pixels, columns: f32) {
        self.extent = Some(HExtent { diff: diff.clone(), fixed, columns });
    }

    /// The wrapping list's state, reset when the diff or split mode changed.
    /// A reload of the same file (`!new_key`) stays at the same row.
    pub fn wrap_list_for(&mut self, diff: &Arc<Diff>, split: bool, n: usize, new_key: bool) -> ListState {
        let same = self.wrap_for.as_ref().is_some_and(|(d, s)| Arc::ptr_eq(d, diff) && *s == split);
        if !same || new_key {
            let top = self.wrap_list.logical_scroll_top();
            let keep = !new_key && self.wrap_for.as_ref().is_some_and(|(_, s)| *s == split);
            self.wrap_list.reset(n);
            if keep && top.item_ix < n {
                self.wrap_list.scroll_to(top);
            }
            self.wrap_for = Some((diff.clone(), split));
        }
        self.wrap_list.clone()
    }
}

/// The handle cell under the pointer: that change block's handle lights up.
/// The cell id makes leave/enter order between two cells of the same block
/// harmless.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct HoveredHandle {
    /// First row of the change block.
    pub block: usize,
    pub cell: usize,
}

/// A gutter drag: the row it started on and whether it includes or excludes.
#[derive(Clone, Copy)]
pub struct GutterDrag {
    pub anchor: usize,
    pub include: bool,
}

/// Pointer state of the line-selection gutter (see `line_selection.rs`).
#[derive(Default)]
pub struct DiffSelectUi {
    pub hovered_handle: Option<HoveredHandle>,
    pub drag: Option<GutterDrag>,
}
