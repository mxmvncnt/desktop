//! `impl Workspace` handlers of the diff viewer: options popover, display
//! setters, split-row cache.

use std::sync::Arc;

use gd_git::{Diff, DiffOptions};
use gpui::{Context, Pixels, Point, Window, point, px};

use super::split_diff::SplitDiff;
use crate::workspace::Workspace;

impl Workspace {
    pub fn open_diff_options(&mut self, pos: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        if self.diff_view.menu.take().is_some() {
            cx.notify();
            return;
        }
        self.popover = None;
        self.menu = None;
        self.diff_view.menu = Some(pos);
        self.diff_view.menu_gen += 1;
        self.focus(window, cx);
        cx.notify();
    }

    pub fn close_diff_options(&mut self, cx: &mut Context<Self>) {
        if self.diff_view.menu.take().is_some() {
            cx.notify();
        }
    }

    pub fn set_diff_split(&mut self, split: bool, cx: &mut Context<Self>) {
        if self.settings.diff_split == split {
            return;
        }
        self.settings.diff_split = split;
        self.settings.save();
        // Row indices mean something else in the other mode.
        self.diff_scroll.0.borrow().base_handle.set_offset(point(px(0.0), px(0.0)));
        self.diff_view.hscroll = px(0.0);
        cx.notify();
    }

    pub fn set_diff_wrap(&mut self, wrap: bool, cx: &mut Context<Self>) {
        if self.settings.diff_wrap == wrap {
            return;
        }
        self.settings.diff_wrap = wrap;
        self.settings.save();
        // The other mode uses another list; start both at the top.
        self.diff_scroll.0.borrow().base_handle.set_offset(point(px(0.0), px(0.0)));
        self.diff_view.wrap_for = None;
        cx.notify();
    }

    pub fn set_ignore_whitespace(&mut self, on: bool, cx: &mut Context<Self>) {
        if self.settings.diff_ignore_whitespace == on {
            return;
        }
        self.settings.diff_ignore_whitespace = on;
        self.settings.save();
        self.apply_diff_options(cx);
        cx.notify();
    }

    /// Push the persisted options into the repository model (reloads diffs if changed).
    pub fn apply_diff_options(&mut self, cx: &mut Context<Self>) {
        let opts = DiffOptions { ignore_whitespace: self.settings.diff_ignore_whitespace };
        if let Some(repo) = &self.repo {
            repo.update(cx, |r, cx| r.set_diff_options(opts, cx));
        }
    }

    /// Paired rows for `diff`, built once per loaded diff and cached.
    pub fn split_for(&mut self, diff: &Arc<Diff>) -> Arc<SplitDiff> {
        if let Some(s) = &self.diff_view.split_cache {
            if Arc::ptr_eq(&s.diff, diff) {
                return s.clone();
            }
        }
        let s = Arc::new(SplitDiff::build(diff));
        self.diff_view.split_cache = Some(s.clone());
        s
    }
}
